This is a interesting project

## Hackathon submission

- Event page: https://www.mlh.com/events/solana-hackathon
- Submission form (sign in with your MLH account): https://www.mlh.com/events/solana-hackathon/submissions/new
- Deadline: **Nov 8, 1:00 PM EST**

### Submitting with an AI agent (DevRelay)

Paste this prompt into your coding agent (e.g. Claude Code) from the repo root:

```text
Submit the project in this repo to Solana Hackathon on MLH (event slug: solana-hackathon) using the DevRelay MCP server.

If you don't have DevRelay's tools yet, install it first by running:
  macOS or Linux: curl -fsSL https://devrelay.com/install.sh | sh
  Windows (PowerShell): iwr -useb https://devrelay.com/install.ps1 | iex
Then stop and tell me to restart you and paste this prompt again, so you pick up the new tools.
If DevRelay says my MLH account isn't connected, call connect_mlh_account and wait while I sign in.

1. Fill in the project from this repo: a name, a one-line pitch (short_description), a description based on the README, the technologies the code uses (including any AI APIs or models it calls, like Gemini or OpenAI), the GitHub link, and any live demo or demo video links you find. Put any other links, like a second repo or slides, in extra_links with a short label. Ask me for anything you can't find.
2. Fill in ai_tools: your own harness and the model you're running on (for example claude-code and coding:anthropic/claude-opus-5.5), nothing else. AI APIs the project calls go with the technologies.
3. Enter it in the prize categories it fits: Top 3 Overall; Best Beginner Hack; Gaming Track Top Prize; DeFi Track Top Prize; AI Track Top Prize. For each one, write a sentence or two on how the project uses it and send it as that category's sponsor_usage answer. Skip any it doesn't fit.
4. Submit it before Nov 8, 1:00 PM EST. If DevRelay's tools don't accept short_description, ai_tools, sponsor_usage or extra_links yet, submit what they do accept, then give me the pitch and category answers so I can paste them into the form. Tell me exactly what you submitted and give me the project's link.
```
