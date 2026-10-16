import { useMemo, useState } from "react";
import type { PresetFamily, UseCase } from "../types";
import { CameraPreview } from "./CameraPreview";

const WORKFLOWS = [
  { slug: "text-to-video", title: "Create video", icon: "▷", description: "Turn an idea into a cinematic shot.", category: "Video" },
  { slug: "text-to-image", title: "Create image", icon: "▧", description: "Find the frame you imagined.", category: "Image" },
  { slug: "image-to-video", title: "Animate image", icon: "◈", description: "Give your stills a sense of motion.", category: "Video" },
  { slug: "edit-video", title: "Edit video", icon: "⌁", description: "Reimagine footage with a prompt.", category: "Video" },
  { slug: "edit-image", title: "Edit image", icon: "✧", description: "Transform a photo with references.", category: "Image" },
];

export function Explore({ useCases, presets, onWorkflow, onStoryboard, onPreset, onAllPresets }: {
  useCases: UseCase[];
  presets: PresetFamily[];
  onWorkflow: (slug: string) => void;
  onStoryboard: () => void;
  onPreset: (preset: PresetFamily) => void;
  onAllPresets: () => void;
}) {
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState("All tools");
  const [hot, setHot] = useState<string | null>(null);
  const workflows = useMemo(() => WORKFLOWS.filter(w =>
    useCases.some(u => u.slug === w.slug) &&
    (category === "All tools" || w.category === category) &&
    `${w.title} ${w.description}`.toLowerCase().includes(query.toLowerCase())
  ), [useCases, category, query]);
  const cameras = presets.filter(p => p.category === "camera-control").slice(0, 6);
  return <main className="explore" aria-label="Explore creation tools">
    <div className="explore-heading"><div><p className="eyebrow">YOUR IDEAS. IN MOTION.</p><h1>What will you create?</h1></div><span className="explore-note">Your models. Your keys. Your creative freedom.</span></div>
    <div className="feature-grid">
      <button className="feature-card feature-cinema" onClick={onStoryboard}>
        <img src="/art/cinema.jpg" alt="Astronaut in a volcanic landscape beneath a glowing planet" />
        <span className="feature-tag">MULTI-SHOT FILMMAKING</span>
        <span className="feature-copy"><span className="feature-kicker">FROM FIRST FRAME TO FINAL CUT</span><strong>Cinema starts<br />with a story.</strong><span>Plan, direct, and assemble your next sequence.</span><span className="feature-cta">Open Storyboard <span>↗</span></span></span>
      </button>
      <button className="feature-card feature-image" onClick={() => onWorkflow("text-to-image")} disabled={!useCases.some(u => u.slug === "text-to-image")}>
        <img src="/art/portrait.jpg" alt="Fashion portrait in chrome and amber against a blue sky" />
        <span className="feature-tag">IMAGE STUDIO</span>
        <span className="feature-copy"><strong>Make it<br />unmistakable.</strong><span>Generate a new perspective.</span><span className="feature-cta">Create image <span>↗</span></span></span>
      </button>
    </div>
    <div className="explore-section-head"><div><h2>Your creative toolkit</h2><p>One place to go from an idea to something worth watching.</p></div><label className="tool-search"><span aria-hidden="true">⌕</span><input type="search" aria-label="Search creation tools" placeholder="Search tools…" value={query} onChange={e => setQuery(e.target.value)} /></label></div>
    <div className="tool-filters" aria-label="Tool category">{["All tools", "Video", "Image"].map(c => <button key={c} aria-pressed={category === c} onClick={() => setCategory(c)}>{c}</button>)}</div>
    <div className="tool-grid">{workflows.map(w => <button className="tool-card" key={w.slug} onClick={() => onWorkflow(w.slug)}><span className="tool-icon" aria-hidden="true">{w.icon}</span><strong>{w.title}</strong><span>{w.description}</span><span className="tool-arrow" aria-hidden="true">↗</span></button>)}</div>
    {workflows.length === 0 && <p className="tool-empty" role="status">{useCases.length ? "No tools match your search. Try another term or category." : "Loading available creation tools…"}</p>}
    {cameras.length > 0 && <><div className="explore-section-head"><div><h2>Direct every movement</h2><p>Explore camera moves. Hover to preview, select to create.</p></div><button className="text-action" onClick={onAllPresets}>Explore presets ↗</button></div><div className="explore-presets">{cameras.map((p, i) => <button key={p.id} className="explore-preset" onMouseEnter={() => setHot(p.id)} onMouseLeave={() => setHot(null)} onFocus={() => setHot(p.id)} onBlur={() => setHot(null)} onClick={() => onPreset(p)}><CameraPreview slug={p.id} src={i % 2 === 0 ? "/art/cinema.jpg" : "/art/portrait.jpg"} active={hot === p.id} /><span>{p.displayName}<span aria-hidden="true">↗</span></span></button>)}</div></>}
    <footer className="explore-footer"><span>Hickeyfield · Independent, open-source creative studio</span><span>Not affiliated with Higgsfield.</span></footer>
  </main>;
}
