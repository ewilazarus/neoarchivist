---
name: ask
description: Ask the docs vault a question, such as how something works here, what happened, what was decided and why, or what's still open. neoarchivist:agent looks it up and answers with links to the notes it rests on.
argument-hint: "<question>"
disable-model-invocation: true
context: fork
agent: neoarchivist:agent
background: false
---

# Ask the vault

You're running this as neoarchivist:agent, for the user's `/neoarchivist:ask` command. Answer the
user's question below under the recall rules, in Markdown for them to read, not JSON: the
answer, the `[[links]]` it rests on, and any disagreements you found. If there is no
question, ask what they want to know.

The question: $ARGUMENTS
