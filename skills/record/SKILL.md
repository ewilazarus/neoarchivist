---
name: record
description: Record what this conversation produced in the docs vault, through neoarchivist:agent. It sends the agent the outcomes that git can't show, and the agent decides what, if anything, to write.
argument-hint: "[anything to emphasise]"
disable-model-invocation: true
---

# Record this work

Write down what this conversation produced, through the `neoarchivist:agent` agent.
You never write to the vault yourself: send the agent a `record` request as its whole task,
one JSON object and nothing else, and run it in the background.

Load the `neoarchivist:contract` skill for the request's format. Build the request from
this conversation: git shows the agent what changed in the repository, so send what git
can't show, meaning the facts that changed, the decisions and why, what was dropped, what's
left open, which follow-ups were finished or reopened, and which notes proved wrong.

The user added: $ARGUMENTS

If nothing in the conversation fits any list, because it was routine work git explains,
tell the user there's nothing to record, and send nothing. Otherwise tell them it's
recording in the background. When the agent replies, report what it wrote in a line or
two. If it asks questions, ask the user them word for word, and send the answers to the
same agent with SendMessage, as the contract describes.
