## Reviewy Overview

Reviewy (working title) is a TUI tool that allows a user to quickly view a project's git diff using difftastic ([https://github.com/wilfred/difftastic](https://github.com/wilfred/difftastic)) as the primary diffing tool. It additionally has a commenting functionality similar to github's web pr review feature which allows a user to select lines and add a comment associated with the lines.

The purpose for building the tool is to give developers a quick and easy way to locally review AI agent code changes in a project. Current best practice is to get agents to produce a PR which is then reviewed. The agent then reads the developer's review comments and makes changes and merges. However, this is a clunky process at best and due to the loop often ends up being left until the end with the developer preferring to instead test function rather than code until review. This can often lead to large, difficult to review prs. One of the most important things is that the developer understand the code that is in production. Now that agents are writing more of it, that requirement has not gone away. Reviewy exists to provide the least friction for reviewing machine written code and commenting locally, long before the formal review process takes place.

### Requirements.

- Written in Rust (external crates are fine)
- Uses Difftastic as the diff tool as it creates the easiest to read (IMO) output.
  - This should use Difftastic's cli (ideally via the difft --display json)
- UI allows for selection of lines/text and creation of comments
- Comments stored as a temporary file or sent directly to the AI session.
  - Comments need to hold information about the file, line(s) and change that was commented on.
- Syntax highlighting via treesitter with themes for colorscheme.
- File explorer that displays currently changed files.
- Side by side 2 column view for diffs. Should display entire file.

#### Stretch Requirements.

- Vim mode.

### Crates.

TUI Framework: Ratatui. we want full control of text rendering and modals.

Git Access: running git commands - we should use the tools that the user has available, not install extra things.

Comment editing: tui-textarea seems fine. Vim bindings for comments is a bit of a stretch. The goal is to make this as friction free as possible, so opening Vim or a text editor is definitely overkill in this case. Simple modal commands would be a plus for me personally, but really, that's a stretch goal.

Syntax Highlighting: tree-sitter integration feels like a really good thing to have as it allows for _proper_ syntax highlighting that matches what people tend to see in their text editor. Depending on implementation issues, I'd try and go with that unless it becomes massively complicated.

### Usage Overview

Here follows an idealized workflow example of how Reviewy should work at first pass.

1. The agent has modified a set of files and reported that the change has been made inside a project.
2. The developer opens a new terminal window in the project directory and runs `reviewy`. Default arguments should diff the current uncommitted changes with the previous branches HEAD.
3. The TUI opens. Initially we see a single panel listing changed files in an explorer format, these are color coded for "modified", "new" and "deleted". On selecting a file, the panel disappears and is replaced with a side by side, two column view of the whole file. This is syntax colored as per the theme (to keep it as close to the editor experience as possible) but with background changed in areas that contain changes.
4. Developer is able to move through the file as if it were a text editor. It is read-only though. Dev is able to select words or lines and hit "comment". This brings up a comment interface that allows a comment to be added to that selection. It also marks the lines as "commented"
5. Dev should be able to select previous comments and edit them by moving to the associated lines and hitting "comment" again. We should not currently allow for multiple comments over the same lines.
6. Once happy, developer closes the application. A comments.md file is created which can either be pasted directly into the agent window, or used as part of a harness.
7. For first version, there is no real persistence. If a comments file exists on app load, a dialog appears asking if the comments want to be deleted or added. If added, lines with comments are marked and become editable in the current session. In both cases, the file is overwritten on exit.

Later versions could well run notifiers and detect changes, stale comments etc. But for the first version, this is overkill.
