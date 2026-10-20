## Reviewy Overview

Reviewy (working title) is a TUI tool that allows a user to quickly view a project's git diff using difftastic ([https://github.com/wilfred/difftastic](https://github.com/wilfred/difftastic)) as the primary diffing tool. It additionally has a commenting functionality similar to github's web pr review feature which allows a user to select lines and add a comment associated with the lines.

The purpose for building the tool is to give developers a quick and easy way to locally review AI agent code changes in a project. Current best practice is to get agents to produce a PR which is then reviewed. The agent then reads the developer's review comments and makes changes and merges. However, this is a clunky process at best and due to the loop often ends up being left until the end with the developer preferring to instead test function rather than code until review. This can often lead to large, difficult to review prs. One of the most important things is that the developer understand the code that is in production. Now that agents are writing more of it, that requirement has not gone away. Reviewy exists to provide the least friction for reviewing machine written code and commenting locally, long before the formal review process takes place.

### Requirements.

- Written in Rust (external crates are fine)
- Uses Difftastic as the diff tool as it creates the easiest to read (IMO) output
- UI allows for selection of lines/text and creation of comments
- Comments stored as a temporary file.

#### Stretch Requirements

- Modal keyboard interface (Vim-like)
- Text/syntax coloring and change highlighting as per a text editor (themes would be ideal)
- Link to active claude code/agent process, so that comments don't need to be saved as file and instead can be sent to the process directly as a review.
- User selectable diffing tool

Let's discuss options for pulling in Difftastic and Possibly TUI (and other) cargo crates. Then let's think about general architecture.

## Notes following output.

#### 1. Pulling in Difftastic.

I initially thought that pulling the crate in would be the way to go. However, thinking of the future, the idea of being able to hook up any diffing tool and to parse its output feels like the best and most flexible fit for the tool. Running the actual tool's command seems the best way to go in either case.

Questions:

- Does git's built in diff also output as json (or similar)? What would it take to be able to select a tool based on what is currently installed?
- Would it work better as a trait/api? We build a component to ingest/parse the diff tool's output and push a standard structure to the renderer. That way, we could build a parser for normal git diff and one for difftastic etc.
- The only issue remains what to do about breaking changes with a package that we don't control... suggestions?

#### 2. Crates

TUI Framework: Ratatui. we want full control of text rendering and modals.

Git Access: running git commands - we should use the tools that the user has available, not install extra things.

Comment editing: tui-textarea seems fine. Vim bindings for comments is a bit of a stretch. The goal is to make this as friction free as possible, so opening Vim or a text editor is definitely overkill in this case. Simple modal commands would be a plus for me personally, but really, that's a stretch goal.

Syntax Highlighting: tree-sitter integration feels like a really good thing to have as it allows for _proper_ syntax highlighting that matches what people tend to see in their text editor. Depending on implementation issues, I'd try and go with that unless it becomes massively complicated.

Filesystem watching: notify crate. We need to be careful about complexity creep here, however.

Supporting crates:

- `color-eyre` or `anyhow` for errors: Explain the differences between these two and then we can pick.

Let's ignore the general architecture for now and focus on the above.

## Usage Overview

Here follows an idealized workflow example of how Reviewy should work.

1. The agent has modified a set of files and reported that the change has been made.
2. The developer opens a new terminal window in the project directory and runs `reviewy`. Default arguments should diff the current uncommitted changes with the previous commit on the branch.
3. The TUI opens. Initially we see a single panel listing changed files in an explorer format, these are color coded for "modified", "new" and "deleted". On selecting a file, the panel disappears and is replaced with a side by side, two column view of the whole file. This is syntax colored as per the theme (to keep it as close to the editor experience as possible) but with background changed in areas that contain changes.
4.

## Some "nice to haves" (in no specific order)

A "go to implementation/reference/declaration" function that opens the file in $EDITOR and then calls the above on the currently selected word. Might be very difficult to do depending on how a particular editor's cli works. The alternative would be building a project file explorer and LSP engine into the app itself, which is WAY OUT OF SCOPE for a reviewing tool!

The simplest version of the above is "open file in Editor" which should be simple enough.

# Answer questions

1. Default should be "all changed files since last commit on this branch" this includes unstaged files.
2. see 1.
3. Yes, the args should attempt to mirror those used by git diff. Let's punt this for now though and flag to discuss later.
4. binary files and renames should be listed. Very large files - we should attempt to view these but this should be behind a button/dialog. i.e. large file, do you want to view? Additionally, we should consider a reviewy.toml configuration file that would allow us to specify files/filetypes to ignore etc.
5. Ideally, this should be modal, like Vim. Normal mode: h and l move columns and j and k move up and down by line. Hit enter and a cursor appears at the beginning of the selected line (and current column). hjkl now move the cursor as if in a normal vim editor. Hit v and we enter visual mode where we can select chars from the cursor. shift-v is visual line (allows us to select multiple lines). For example: enter enter opens a comment box with the line tagged. enter shift-v enter opens the comment box with the line tagged and pastes the line into the box. enter w w v w enter opens the comment box with the third word tagged and pastes that word into the comment box.
6. Wrap by default - this should be settable in the toml
7. Again, vim like commands: g] next hunk g[ prev hunk gn next file gp prev file
8. That seems adequate.
9. We need to create the comments.md in such a way that it is easily parseable by the agent. There will already be a serializer as there is an in-memory representation. We want this serialization to be lossless so a deserializer should be simple to write too.
10. .reviewy/ this whole folder should be gitignored by default.
11. Ignore direct send for now. Let's concentrate on the file version.
12. v1 Rust, Python, C#, Typescript
13. What is the theme setup for zed. Ideally for v1 we should be able to parse zed themes.
14. one crate. reviewy-core as you state uses a ton of external deps which makes testing hard in any case.
15. sync. Async gives us nothing but complexity.

Let's discuss the difft issues in more detail too. What are the alternatives for difftastic? What do things look like if we use the regular git difftool?

1. That's fine. We want this tool to enable more, smaller reviews of agent work. The purpose is to keep the loop as tight as possible. An extra file or two due to a rename is fine.
2. Esc steps back one level, enter on a line which already has a comment opens that comment for editing. We need to be careful about multi-line comments that start on uncommented lines and then move over a commented one (see below). Selections cannot cross columns. No search in v1 but will be priority for v2.
3. Gutter marker for v1. We can spike previews once v1 is done. Previews are difficult to get right from a UX point of view.
4. Rather than sticking with this as a format, let's take a step back and think about the best format for the job. Is .md ideal? What other file types would serve the purpose better? JSON, TOML? Something else? The important part is that they should be easily readable by LLM (so that it can act on the comments) and contain all the information it would need to do so. Additionally, it should be easily serializable/deserializable. Let's discuss options.
5. .reviewy/ with a global at .config/reviewy. This is a user preference, not a project preference.
6. We bundle a theme with the binary. How do themes work for other TUIs? Is a link to the theme in the config normally acceptable?
7. Both. We need to avoid issues with strange obfuscated files (which LLMs can actually edit) that are single line/few lines but huge.

We need to also discuss what happens if a user comments on line 10, then selects from lines 5 to 15 and hits enter. Ideally, we should have 2 comments, one overlapping the other. This could cause issues though. For v1 we should handle this a simply as possible. Options?

---

1. Delete with D - needs confirmation modal. Empty body also leads to no comment being saved.
2. Comments are saved automatically as they are typed. No manual save. Esc should pop up modal asking if user wants to close the comment (to avoid quick Esc taps to get to normal mode from insert closing the comment). If comment body is empty, nothing is saved. Otherwise comments are just saved. User can manually delete comments if they need to.
3. Quit is ctrl-q. Needs confirmation. No comments wipes file. Note that if comments were already present, the User was asked if they wanted to keep them on load - so there is no way of accidentally deleting comments without confirmation.
4. This should be in the metadata - old side comment. The default preamble should explain this (see below)
5. Default preamble should be provided at the top of the md. config TOML should allow for a replacement string/block to be used instead.
6. 5k lines, 1Mb, 1k char line limit. Anything more isn't really human readable - which defeats the whole point of the app.

On other points:
Hybrid seems like a good way to go. We're then maintaining 2 files but that's okay. It also keeps the door open for MCP or other paths to pass information to the LLM without changing the data/serialization/deserialization route.

Bundled themes with a themes folder in the config directory seems good. No to the zed lookup - don't want to couple the themes in case we decide to modify things specifically for this app.

Overlapping comments. Structure for B but hold off the UI implementation for v1 if complicated. That can then be added later without structural changes to the logic/model.

---

Gap addressing:

Writing files every change will generate a ton of churn. This is meant to be a tiny footprint, fast app and writing to disk every keypress is a bad call.

Crashes or panics should aim to serialize the current in memory model to disk, if possible. Otherwise, the serialization should occur on app exit or when a terminate message is received. We already have to run commands on panic due to ratatui's control over the terminal window. A full on panic where even that can't be fixed leads to that terminal session needing to be killed, losing some comments is the least of the problems in that case! We should aim instead to not let things crash ( ;) )!

Esc -> Confirmation Modal: Enter confirms, Esc cancels. That way User needs to hit Esc Enter to close, and Esc Esc if hit by mistake. Simple 2 button combo, but must be explicit.

Only thing to note, if a comment existed but the user deletes all text, we need to remove that comment and its metadata from the model on close. Otherwise, all is correct.

UI: Agree with your take, Picker can be v1.1

Crates and layout:

- comment editor. How difficult will it be to implement modal editing in tui-textarea? This is going to be a priority for v1.1 so we shouldn't paint ourselves into a corner.
- imara-diff and builtin.rs should be punted to v1.1
- consider color-eyre instead of anyhow for app errors.
- What is the purpose of the tempfile crate?

questions:
1. j/k in row mode (not cursor/selection) move by line (not screen row). In select mode (user has hit enter and a cursor is visible) j and k move by screen row (i.e. as if gj and gk).
2. two columns with one empty. New appear in right column, deleted in left. 
3. Esc from row should bring up an explorer modal/column. then hjkl navigate the current file. We need to discuss explorer in more detail. For v1 we can have simple list of file names. Later improvements will display a file hierarchy.
4. see above
5. Implementing a vim mode is going to be a big part of v1.1. For now, simple w/b/e.
6. Yes, but remember we're going to be implementing modal editing in v1.1. We need to avoid reinventing the wheel in multiple places.

---

Saving: the details (notes)

- the panic hook massively complicates the comment model. The app's architecture shouldn't be created to deal with crashes. We should use the simplest data structures and memory types available for the happy path. If a crash causes us to lose comments, then so be it. Later versions of the software can deal with this. 
- Signals can be handled gracefully without the need for mutex locks. Only panics are special and in that case, we accept that data will be lost. We can revisit this at a later date, once we know more about how users (in this case, me) interact with the software.
- ctrl-c should do nothing. In this context, ctrl-c is likely to be used for copy, not kill.

tempfile (notes)

If this is true then how does the following command work: `git -c diff.external=difft diff HEAD`?

Questions:

1. our own, seems like the right path for now.
2. Overlay list, j/k choose. Esc closes list, Enter opens selected file (replaces current in tui). Comment count useful. No changes to preview currently. On app open: same overlay list but blank columns.
3. Add counts if helps with parser design.
4. This is fine. currently this is an app for me, I can ensure this is installed. If it becomes useful and I decide to publish to the public, then we can consider that. But at that time, we'll likely be well beyond v1.1!
5. Message and exit as suggestion.
6. yes. Will require some sort of visual indicator, like a B or icon - similar to the comment count

---
Notes:

"new files skip difft anyway" - we DO want to diff new, untracked files if they have been created. Is there a way of doing this? Some 'include untracked' flag?

Questions:
1. later, but yes, will be required after v1
2. yes.
3. yes for the saved comment information.
4. yes, no checks for v1. For v1. save commit hash with comments - we will later use this to verify if comments could be stale.
5. Yes.
6. Yes, punt for now. 
7. location good, don't read.

One thing left. Note that if a visual selection is made before entering a comment, we should copy that selected text into the comment box - useful for code edits.

---

1. wrapped in fence - will be needed later for syntax highlighting/coloring in comments (not v1) and for change interface. Currently we shouldn't assume "replace with this" unless the comment says so.
2. Yes.

Happy to start spec writing now unless you have further questions.

---

Quick notes:

Exiting comments: Esc -> Confirmation Modal: Enter confirms, Esc cancels. That way User needs to hit Esc Enter to close, and Esc Esc if hit by mistake. Simple 2 button combo, but must be explicit.


---

Most urgent additions (pre v1)

- move to next comment (similar keybind to move to hunk) `g] g[ best candidates` 
- syntax highlighting of comments
- Modal editing in comments
- mouse interaction.
