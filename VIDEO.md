# Demo video

Spec for the hackathon demo video. Hand this file to a coding agent (e.g. Claude Code) once the app runs.

## Target

- Length: **under 60 seconds**
- Output: `demo.mp4`, 1920x1080, 30 fps, H.264
- Look: app shown inside a macOS-style window on a wallpaper background
- Motion: zoom in / zoom out on key moments, starting in the intro
- No captions or voiceover unless added below

## Before recording (fill these in)

- Run command: `TODO` (e.g. `npm run dev`)
- URL: `TODO` (e.g. `http://localhost:3000` or the live demo link)
- Intro shot: `TODO` (what the first ~5s shows)
- Demo flow, in order (3–5 steps):
  1. `TODO` (e.g. connect wallet)
  2. `TODO` (e.g. view balance)
  3. `TODO` (e.g. send a transaction)
  4. `TODO` (e.g. show confirmation on Solana explorer)

## Pipeline

1. **Record**: start the app, then drive the flow above with a scripted browser (`npx playwright`, not added as a project dependency) recording at 1920x1080. Keep pauses short; aim for ~50s raw.
2. **Mac window frame**: with `ffmpeg`, scale the recording down, round its corners, add a title bar with the red/yellow/green traffic lights and a soft drop shadow, and overlay it centered on a wallpaper.
3. **Zooms**: with `ffmpeg` `zoompan`/`crop`+`scale`, start wide on the full window in the intro, zoom into the element that matters in each step, and zoom back out between steps. Ease in/out, ~0.5s per zoom.
4. **Trim and export**: cut dead time, confirm the duration is under 60s with `ffprobe`, export `demo.mp4`.
5. **Submit**: upload to YouTube (unlisted is fine) and add the link to the MLH submission as the demo video.

## Deadline

MLH Solana Hackathon submissions close **Nov 8, 1:00 PM EST**. Record as soon as the flow works.
