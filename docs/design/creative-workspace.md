# Creative workspace redesign — 2026-10-16

Reference inspected: https://higgsfield.ai/ and its publicly documented creation workflows. The application retains its independent Hickeyfield identity. All shipped copy and artwork are original.

Implemented: default Explore hub, original cinematic feature artwork, searchable/category-filtered workflow cards, direct Image/Video navigation, animated camera-preset discovery, responsive dark workspace with lime controls, less crowded composer, aspect-preserving result preview with separate metadata, and generated-image-to-animation handoff. Existing provider setup, capability-aware models, pricing, prompt enhancement, storyboard planning, continuity, rendering, and library remain wired to the existing core. Browser preview now exposes the same five use-case shapes instead of an empty navigation; generation there remains sample data, as labeled in the header.

## Feature coverage

| Workflow | Status |
| --- | --- |
| Text-to-video, image-to-video, video editing | Existing provider-backed workflows, now directly discoverable |
| Text-to-image, image editing | Existing provider-backed workflows, now directly discoverable |
| Camera directions and presets | Existing engine, new visual entry points and hover previews |
| Generated image to video | Added result action that attaches the image as a start frame |
| Multi-shot filmmaking | Existing Storyboard with model-aware planning, chaining, continuity checks and local assembly |
| Library and rerun | Existing local library; rerun now opens the composer |
| Audio/voice generation, voice cloning, translation | Not implemented as dedicated workflows |
| Persistent character / influencer identity | Not implemented as a dedicated workflow |
| Motion transfer, reframe, upscale | Some catalogue entries may exist; no dedicated capability-validated workflow is supplied in this change |
| Agentic editing, advertising studios, timeline editing | Not implemented |
| Hosted collaboration, community and publishing | Not implemented |

This is not full Higgsfield feature parity. The remaining tools need dedicated input contracts, provider routing, persistence, and end-to-end validation; adding navigation alone would not implement them. No provider generation was purchased during validation.

## Artwork

Generated with the built-in imagegen tool; JPEG copies are bundled under `ui/public/art/` and work offline. These are editorial navigation illustrations, not claimed model outputs.

- `cinema.jpg`: “Create one ultrawide cinematic editorial photograph for the hero banner of a creative filmmaking app. A lone astronaut in an ivory suit standing on dark volcanic dunes, small human figure positioned at the right third, enormous luminous orange-red planet suspended above the horizon, smoky dusk atmosphere, photorealistic 35mm film grain, warm amber rim light, black and copper palette, dramatic luxury science fiction movie still. Left half mostly dark open landscape with generous space for UI text to be overlaid separately. Landscape aspect ratio 3:2 or wider. No text, no letters, no logos, no interface.”
- `portrait.jpg`: “Photorealistic high fashion editorial portrait, close-up of a young adult woman with short black hair in a sculptural chrome silver jacket, wind blowing hair, futuristic amber wraparound sunglasses, sunlit cobalt blue sky background, hard sunlight, luxurious magazine photography, tactile skin, analog grain, bold blue silver orange color palette, vertical 3:4 composition, no text, no logos. Original preview artwork for AI image creation studio.”

## Platform-wide follow-through

Image, Video, and Studio now have mutually exclusive navigation states. Image and Video filter recent creations by modality; Studio retains the combined workspace. The composer is organized into model, prompt, references/source media, creative direction, and output settings, with an independently scrolling body and a fixed generation footer on desktop. Less-used controls live in Advanced settings. The inert Elements control was removed.

Storyboard now separates the creative brief from the sequence canvas, provides three editable brief starters, and presents draft shots as numbered cards with prompt, duration, model, and camera controls. Continuity options collapse so they do not cover the working sequence. Library adds combined text search, media filters, and date sorting. The inspector follows the selected result and updates when a search filters it out. Automatic scrolling between two parallel metadata lists was removed because there is now one focused inspector.

Model selection, presets, provider setup, lightboxes, and empty states use the same typography, surfaces, and focus styling. Model search supports names, descriptions, and providers. Browse-mode sample results use the original bundled artwork; video examples show a preview still and do not claim to provide playable footage.

Visual checks cover 1440px desktop, 1100px compact desktop, 760px tablet, and 390px narrow layouts. Generation was not purchased during validation.
