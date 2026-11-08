# Story Integration

The source of truth for the in-game novel is `Story.md` at this directory's root.
The Rust compiler embeds it directly using `include_str!("../Story.md")` in `src/story.rs`.

## Progression

The new-campaign prologue is presented before the first gameplay briefing. Each act's briefing uses its actual opening prose as a spoiler-free teaser. Completing an act automatically opens its full manuscript chapter, then returns to the debrief; players can press Esc to skip reading. Clearing the final encounter presents Act V and then the full epilogue.

The story library is accessible with B at any time, including from the title screen, paused game, mission briefings, the evidence journal, and the ending. Completed-act chapters remain unlocked through the existing `MATA_CAMPAIGN_V2` checkpoint. Pre-disaster vignettes open after Act III; the epilogue and evidence index open after Act V. The future acts' content is never exposed through the library prematurely.

## Implementation and maintenance

- `src/story.rs`: chapter markers, full text embedding, Markdown-to-lines conversion and safe character-width wrapping.
- `src/campaign.rs`: story reader/library screens, chapter locks and mission-story transitions, preserving the existing save format.
- `src/render.rs`: native-resolution, dynamically paginated full-screen story UI, with cached wrapped lines per chapter and viewport width.
- `src/main.rs`: story navigation keys and unchanged FPS controls.

To revise the manuscript, edit `Story.md` and recompile. Keep the nine top-level section headings matched by the `MARKERS` table in `src/story.rs`. No extra dependencies are introduced.
