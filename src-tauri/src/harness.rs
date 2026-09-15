//! The harness: turning what the user selected into the prompt actually sent.
//!
//! `enhance.rs`, `preset.rs`, `camera.rs`, `enhancer.rs` and the `prompts/`
//! corpus were all complete, tested, and **called by nothing**. 371 green tests
//! were exercising a subsystem no user path reached, so selecting a camera
//! preset changed the id stored in SQLite and nothing else — the provider
//! received the raw typed prompt with no camera clause and no rewrite.
//!
//! This module is the missing caller. It lives in the shell rather than the
//! core because it needs the user's settings and their choice of rewriter,
//! and it is deliberately one function so the ordering hazards below are
//! decided in exactly one place.
//!
//! **Order matters and is not arbitrary:**
//!
//! 1. Resolve the preset *first* — the enhance decision needs to know whether a
//!    real one was chosen (rule 1 forces enhancement on).
//! 2. Build the camera clause into [`PromptParts`], but hand the rewriter only
//!    the **scene**. Passing the compiled prompt still "works" and quietly
//!    degrades every generation: the model rewrites our five-slot camera
//!    grammar into prose and the preset's precision is lost.
//! 3. Decide *whether* to enhance before running anything, because an end frame
//!    forbids it unconditionally.
//! 4. Recompile after the rewrite, so the camera clause survives verbatim.

use hickeyfield_core::enhance::{self, EnhanceInputs, PresetSelection, PromptParts};
use hickeyfield_core::enhancer::{
    enhance_or_original, mode_for, recipe_pin, EnhanceRequest, Enhancer, HostedBackend,
    HostedEnhancer, LocalEnhancer, Mode, RewriteStatus, Rewritten,
};
use hickeyfield_core::{corpus, MediaRef, Model};

/// What the harness produced, and how.
pub struct Compiled {
    /// The string to send to the provider.
    pub prompt: String,
    /// The user's own words, always preserved so the UI can show both.
    pub original: String,
    /// `Some` only when a rewriter actually changed the text.
    pub enhanced: Option<String>,
    /// Which corpus and rewriter ran. `None` when none did — never a
    /// placeholder, because a guessed pin makes two unlike generations look
    /// reproducible.
    pub version: Option<String>,
    /// Shown next to the toggle when the rewrite did not happen or failed.
    pub note: Option<String>,
}

/// Which rewriter to use, chosen by the user (or auto-selected in the shell).
pub enum Rewriter<'a> {
    /// No rewrite. The compiled prompt still gets its camera clause.
    None,
    /// Local Ollama. Free, private, no key.
    Ollama { model: &'a str },
    /// The user's own hosted key. `backend` selects the wire dialect; the key is
    /// read from the vault by the shell and borrowed in, never stored here.
    Hosted {
        backend: HostedBackend,
        api_key: &'a str,
        model: &'a str,
    },
    /// Enhance was wanted but no backend is available. Carries the honest note
    /// authored in `commands.rs`. `compile` surfaces it only when the three
    /// enhance rules leave enhancement on, so it never masks the end-frame
    /// reason.
    Unavailable { note: String },
}

/// The shared tail every real rewrite runs: call the enhancer, and on success
/// put the rewritten scene back through `PromptParts::compile` so the camera
/// clause is re-appended verbatim.
///
/// Local and Hosted differ only in the concrete [`Enhancer`] and the
/// `(provider, model)` pair recorded in the version pin — everything after that
/// is identical, so it lives here in exactly one place.
fn finish_rewrite(
    enhancer: &dyn Enhancer,
    req: &EnhanceRequest,
    parts: &PromptParts,
    mode: Mode,
    pin: (&str, &str),
    original: String,
    compiled_now: String,
) -> Compiled {
    let out: Rewritten = enhance_or_original(enhancer, req);
    match out.status {
        RewriteStatus::Rewritten => {
            // Put the rewritten scene back and recompile, so the camera clause
            // is appended verbatim to the improved prose.
            let final_prompt = PromptParts {
                scene: out.prompt.clone(),
                ..parts.clone()
            }
            .compile();
            Compiled {
                prompt: final_prompt,
                original,
                enhanced: Some(out.prompt),
                version: Some(recipe_pin(corpus::CORPUS_ID, mode, Some(pin))),
                note: None,
            }
        }
        // A failed rewrite must never block a generation the user asked for, and
        // must never look like it succeeded.
        _ => Compiled {
            prompt: compiled_now,
            original,
            enhanced: None,
            version: None,
            note: out.note,
        },
    }
}

/// Compile the prompt for one submission.
pub fn compile(
    model: &Model,
    raw_prompt: &str,
    preset_id: Option<&str>,
    media: &[MediaRef],
    enhance_toggle: bool,
    rewriter: Rewriter<'_>,
) -> Result<Compiled, String> {
    // 1. The preset, resolved to a real family rather than an opaque id.
    let family = preset_id.and_then(hickeyfield_core::preset::get);

    // 2. The camera clause. `with_camera` takes the slug and looks up the
    //    five-slot template, so the grammar stays in one place.
    let mut parts = PromptParts::scene(raw_prompt);
    if let Some(f) = family {
        if let Some(cam) = f.camera_template.as_deref() {
            parts = parts.with_camera(cam);
        }
    }

    // 3. Whether to enhance at all. Three rules, and the end-frame one is
    //    unconditional: rewriting a prompt between two fixed frames produces
    //    something that matches neither.
    let decision = enhance::build(
        &parts,
        EnhanceInputs::new(model.job_type)
            .with_preset(PresetSelection::from_family(family))
            .with_end_frame(hickeyfield_core::media::has_end_frame(media))
            .with_toggle(enhance_toggle),
        None,
    );

    let original = raw_prompt.to_string();
    let compiled_now = decision.prompt.clone();

    if decision.has_unresolved_sentinel {
        return Err(
            "this prompt still points at an attachment that has not been bound".to_string(),
        );
    }

    if !decision.enhance {
        return Ok(Compiled {
            prompt: compiled_now,
            original,
            enhanced: None,
            version: None,
            note: Some(decision.reason.explanation().to_string()),
        });
    }

    // Dispatch on the chosen rewriter. `None` and `Unavailable` return the
    // sendable prompt with an honest note *before* any network work; the two
    // real backends fall through to the shared rewrite tail below. The
    // `Unavailable` note is authored in commands.rs (AC2) and only reaches the
    // user here, when the three enhance rules have left enhancement on.
    enum Backend<'a> {
        Ollama(&'a str),
        Hosted(HostedBackend, &'a str, &'a str),
    }
    let backend = match rewriter {
        Rewriter::None => {
            return Ok(Compiled {
                prompt: compiled_now,
                original,
                enhanced: None,
                version: None,
                note: Some("no rewriter selected — sent as written".to_string()),
            });
        }
        Rewriter::Unavailable { note } => {
            return Ok(Compiled {
                prompt: compiled_now,
                original,
                enhanced: None,
                version: None,
                note: Some(note),
            });
        }
        Rewriter::Ollama { model: tag } => Backend::Ollama(tag),
        Rewriter::Hosted {
            backend,
            api_key,
            model,
        } => Backend::Hosted(backend, api_key, model),
    };

    // 4. The rewrite. Note it receives `raw_prompt`, NOT the compiled string:
    //    handing it the camera clause invites the model to paraphrase our
    //    five-slot grammar into prose, which loses exactly the precision the
    //    preset exists to supply.
    let roles: Vec<_> = media.iter().map(|m| m.role).collect();
    let mut req = EnhanceRequest::new(
        raw_prompt,
        model.job_type,
        &model.display_name,
        model.modality,
    )
    .with_media(&roles);
    if let Some(f) = family {
        req = req.with_preset(&f.description);
    }

    let Some(mode) = mode_for(&req) else {
        // Audio and 3D have no overlay, and the base corpus must never be used
        // alone. Refusing beats sending shot-grammar guidance to a TTS model.
        return Ok(Compiled {
            prompt: compiled_now,
            original,
            enhanced: None,
            version: None,
            note: Some("no enhancer guidance exists for this kind of model".to_string()),
        });
    };

    let system = corpus::system_prompt_for(mode)?;

    // 5. Build the concrete enhancer and its version pin, then hand off to the
    //    shared tail. Only one branch runs, so `system` is moved exactly once.
    let (enhancer, pin): (Box<dyn Enhancer>, (&str, &str)) = match backend {
        Backend::Ollama(tag) => (Box::new(LocalEnhancer::new(tag, system)), ("ollama", tag)),
        Backend::Hosted(b, key, model) => (
            Box::new(HostedEnhancer::new(b, key, model, system)),
            (b.slug(), model),
        ),
    };

    Ok(finish_rewrite(
        enhancer.as_ref(),
        &req,
        &parts,
        mode,
        pin,
        original,
        compiled_now,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickeyfield_core::{MediaRole, ProviderId};

    fn model(id: &str) -> Model {
        hickeyfield_core::registry::registry()
            .remove(id)
            .unwrap_or_else(|| panic!("{id} missing from the registry"))
    }

    #[test]
    fn a_camera_preset_reaches_the_provider() {
        // The bug this whole module exists for: selecting a preset used to
        // change the stored id and nothing else, so the provider received the
        // raw prompt with no camera clause.
        let m = model("kling3_0");
        let slug = hickeyfield_core::camera::slugs().next().unwrap();
        let out = compile(
            &m,
            "a lighthouse in fog",
            Some(slug),
            &[],
            false,
            Rewriter::None,
        )
        .unwrap();
        assert_ne!(
            out.prompt, "a lighthouse in fog",
            "the preset contributed nothing to the prompt"
        );
        assert!(out.prompt.contains("a lighthouse in fog"), "{}", out.prompt);
    }

    #[test]
    fn an_end_frame_forbids_the_rewrite_even_with_a_preset() {
        // Rule 2 beats rule 1. Interpolating between two fixed frames must not
        // have its prompt rewritten — the result would match neither frame.
        let m = model("kling3_0");
        let slug = hickeyfield_core::camera::slugs().next().unwrap();
        let media = [
            MediaRef::url(MediaRole::Start, "https://a/1.png"),
            MediaRef::url(MediaRole::End, "https://a/2.png"),
        ];
        let out = compile(
            &m,
            "a lighthouse",
            Some(slug),
            &media,
            true,
            Rewriter::Ollama { model: "nope" },
        )
        .unwrap();
        assert!(out.enhanced.is_none(), "an end frame must forbid rewriting");
        assert!(out.version.is_none());
    }

    #[test]
    fn the_users_own_words_are_always_preserved() {
        let m = model("kling3_0");
        let out = compile(&m, "a red door", None, &[], false, Rewriter::None).unwrap();
        assert_eq!(out.original, "a red door");
    }

    #[test]
    fn a_missing_rewriter_still_produces_a_sendable_prompt() {
        // An optional improvement must never block a paid generation.
        let m = model("kling3_0");
        let out = compile(
            &m,
            "a red door",
            None,
            &[],
            true,
            Rewriter::Ollama {
                model: "definitely-not-installed",
            },
        )
        .unwrap();
        assert!(!out.prompt.is_empty());
        assert!(out.enhanced.is_none());
        // And it must say why, rather than looking like it worked.
        assert!(out.note.is_some());
    }

    #[test]
    fn a_version_pin_is_recorded_only_when_a_rewrite_happened() {
        // A guessed pin would make two unlike generations look reproducible.
        let m = model("kling3_0");
        let out = compile(&m, "x", None, &[], false, Rewriter::None).unwrap();
        assert!(out.version.is_none());
    }

    #[test]
    fn the_camera_clause_is_the_five_slot_grammar_not_a_paraphrase() {
        // The preset's value is its precision. If the compiled prompt loses the
        // slot structure, the preset has been reduced to a label.
        let m = model("kling3_0");
        let out = compile(
            &m,
            "a harbour at dusk",
            Some("push-in"),
            &[],
            false,
            Rewriter::None,
        )
        .unwrap();
        let tmpl = hickeyfield_core::camera::get("push-in").unwrap().render();
        assert!(
            out.prompt.contains(&tmpl),
            "expected the rendered template verbatim.\n  got: {}\n want: {tmpl}",
            out.prompt
        );
    }

    #[test]
    fn a_preset_forces_enhancement_on_even_with_the_toggle_off() {
        // Rule 1. The preset's aesthetic is delivered by the rewrite, so
        // selecting one overrides the toggle — that is why the toggle is not
        // simply honoured here.
        let m = model("kling3_0");
        let off = compile(&m, "a harbour", Some("push-in"), &[], false, Rewriter::None).unwrap();
        // With no rewriter the text is unchanged, but the *decision* must have
        // been to enhance — visible in the note, which explains the forcing.
        assert!(
            off.note.as_deref() != Some("enhancement is off for this model by default"),
            "a real preset must not leave the default-off reason in place: {:?}",
            off.note
        );
    }

    /// Answer exactly one HTTP request with a canned JSON body, returning the
    /// bound base URL. A std `TcpListener` keeps the hosted test real with no new
    /// dependency — the same shape the core enhancer tests use.
    fn stub_once(body: &'static str) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            if let Ok((mut sock, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let _ = sock.read(&mut buf);
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = sock.write_all(resp.as_bytes());
                let _ = sock.flush();
            }
        });
        addr
    }

    #[test]
    fn a_hosted_rewrite_enhances_and_keeps_the_camera_clause() {
        // AC1: compile()'s shared tail, driven with a HostedEnhancer pointed at a
        // local stub, produces an enhanced prompt whose camera clause survived
        // the recompile and whose version pins the OpenAI backend + model.
        let m = model("kling3_0");
        let addr = stub_once(
            r#"{"choices":[{"finish_reason":"stop","message":{"content":"A single banana with a wide grin, cinematic key light, shallow depth of field","refusal":null}}]}"#,
        );

        let req = EnhanceRequest::new(
            "a banana with a smile",
            m.job_type,
            &m.display_name,
            m.modality,
        );
        let mode = mode_for(&req).expect("a video job has an overlay");
        let system = corpus::system_prompt_for(mode).unwrap();
        let parts = PromptParts::scene("a banana with a smile").with_camera("push-in");

        let enhancer = HostedEnhancer::openai("sk-test", "gpt-test", system).with_base_url(&addr);
        let out = finish_rewrite(
            &enhancer,
            &req,
            &parts,
            mode,
            ("openai", "gpt-test"),
            "a banana with a smile".to_string(),
            parts.compile(),
        );

        assert_eq!(
            out.enhanced.as_deref(),
            Some("A single banana with a wide grin, cinematic key light, shallow depth of field"),
            "the hosted reply should become the enhanced scene"
        );
        assert!(
            out.note.is_none(),
            "a success carries no note: {:?}",
            out.note
        );
        // The camera clause survived the recompile verbatim.
        let tmpl = hickeyfield_core::camera::get("push-in").unwrap().render();
        assert!(
            out.prompt.contains(&tmpl),
            "the camera template was lost:\n  got: {}",
            out.prompt
        );
        assert!(out.prompt.contains("wide grin"), "{}", out.prompt);
        assert_eq!(
            out.version,
            Some(recipe_pin(
                corpus::CORPUS_ID,
                mode,
                Some(("openai", "gpt-test"))
            )),
            "the version pin must name the hosted backend and model"
        );
    }

    /// AC4 live check, run by a human against the machine's own Ollama:
    ///
    /// ```sh
    /// OLLAMA_ENHANCE_MODEL=gemma3:1b \
    ///   cargo test -p hickeyfield-tauri --lib harness -- --ignored --nocapture
    /// ```
    ///
    /// "a banana with a smile" on an image model with enhance on and a real
    /// Ollama backend must come back as an expanded cinematic prompt, distinct
    /// from the raw text, with a version pin naming the model. Use a
    /// non-reasoning model: `clean_reply` does not strip `<think>` blocks, so a
    /// reasoning model would leak its chain of thought into the prompt.
    #[test]
    #[ignore = "needs a running Ollama daemon; run with --ignored"]
    fn a_banana_gets_a_cinematic_rewrite_against_a_real_daemon() {
        let tag = std::env::var("OLLAMA_ENHANCE_MODEL")
            .expect("set OLLAMA_ENHANCE_MODEL to an installed tag");
        let m = model("nano_banana_2");
        let out = compile(
            &m,
            "a banana with a smile",
            None,
            &[],
            true,
            Rewriter::Ollama { model: &tag },
        )
        .unwrap();
        println!("enhanced: {:?}\nversion: {:?}", out.enhanced, out.version);
        let enhanced = out.enhanced.expect("the daemon must produce a rewrite");
        assert_ne!(
            enhanced, "a banana with a smile",
            "the prompt was not expanded"
        );
        assert!(enhanced.len() > "a banana with a smile".len());
        let version = out.version.expect("a successful rewrite pins its version");
        assert!(version.contains("ollama"), "version: {version}");
        assert!(version.contains(&tag), "version: {version}");
    }

    #[test]
    fn a_hosted_rewriter_with_no_key_falls_back_to_the_original_prompt() {
        // AC1/AC2: an empty hosted key must never block the generation. compile()
        // returns the sendable original with a note and no enhancement — the same
        // contract the Ollama fallback keeps.
        let m = model("kling3_0");
        let out = compile(
            &m,
            "a banana with a smile",
            None,
            &[],
            true,
            Rewriter::Hosted {
                backend: HostedBackend::OpenAi,
                api_key: "",
                model: "gpt-x",
            },
        )
        .unwrap();
        assert!(!out.prompt.is_empty());
        assert!(out.enhanced.is_none(), "no key means no enhancement");
        assert!(out.version.is_none());
        assert!(out.note.is_some(), "the failure must explain itself");
    }

    #[test]
    fn an_unavailable_rewriter_surfaces_its_honest_note() {
        // AC2: the honest "nothing available" note authored in commands.rs is
        // carried through compile() unchanged when enhancement stays on.
        let m = model("kling3_0");
        let note = "Enhance is on, but there is no rewriter to run it. \
                    Your prompt was sent exactly as you wrote it.";
        let out = compile(
            &m,
            "a banana with a smile",
            None,
            &[],
            true,
            Rewriter::Unavailable {
                note: note.to_string(),
            },
        )
        .unwrap();
        assert!(out.enhanced.is_none());
        assert!(out.version.is_none());
        assert_eq!(out.note.as_deref(), Some(note));
    }

    #[test]
    fn every_launch_model_compiles_without_panicking() {
        // job_type, preset lookup and mode selection all index by model; a gap
        // in any of them would panic on a model a new user is shown first.
        for m in hickeyfield_core::registry::launch_models() {
            let out = compile(&m, "a test", None, &[], false, Rewriter::None);
            assert!(out.is_ok(), "{} failed to compile a prompt", m.id);
        }
        let _ = ProviderId::Fal;
    }
}
