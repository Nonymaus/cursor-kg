---

# My Coding Project: To-Do & Improvement Plan

## Quick Overview

This is my plan to clean up and improve my Knowledge Graph MCP Server project. It's a high-performance Rust/Python server that connects to Cursor IDE. Right now, it works and it's fast, which is great, but it's getting a bit messy. The goal is to fix the annoying parts and clean up the foundation so I can add more features without breaking everything.

## Where Things Stand

-   **Architecture**: The Rust and Python parts are a bit tangled. I need to clarify what does what.
-   **Performance**: It's fast! The core idea works well.
-   **Docs**: My notes and setup instructions are scattered across a few different files.
-   **Config**: There are two config files (`config.toml` and `enhanced_config.toml`), which is confusing.
-   **Testing**: I have some tests, but I'm not sure what's covered and what isn't.

## The To-Do List

### Priority 1: Stop the Pain (Fixes that make development easier)

-   [ ] **Merge the Config Files**
    -   [ ] Combine `config.toml` and `enhanced_config.toml` into a single, simple `config.toml`.
    -   [ ] Add some basic checks so the server crashes with a clear error if the config is bad.
    -   [ ] Delete the old config files.

-   [ ] **Make a Good README**
    -   [ ] Create a single `README.md` at the root of the project.
    -   [ ] Write down the steps to install dependencies and run the server from scratch.
    -   [ ] Add a section with all the possible settings for the new `config.toml`.

### Priority 2: Clean Up The Foundation (So I don't regret this later)

-   [ ] **Clean up the Rust/Python Bridge**
    -   [ ] Add comments explaining what Python is responsible for vs. what Rust does.
    -   [ ] Make sure the data being passed between them is simple and well-defined.

-   [ ] **Add Some Basic Tests**
    -   [ ] Write a few tests for the most important or tricky logic (like the hallucination detection).
    -   [ ] Create one end-to-end test that simulates a real request to make sure the whole thing works together.

-   [ ] **Make It Easier to Run**
    -   [ ] Create a simple Dockerfile so I don't have to install all the dependencies manually next time.
    -   [ ] Add a simple `/health` endpoint that just returns `{"status": "ok"}`.

### Priority 3: The Fun Stuff (New Features!)

-   [ ] **Improve Hallucination Detection**
    -   [ ] Look into better ways to verify facts or detect contradictions.
    -   [ ] Add a way to measure how confident the model is in its answer.

-   [ ] **Optimize Codebase Indexing**
    -   [ ] Make indexing faster for really big codebases.
    -   [ ] Add support for another programming language.
    -   [ ] Figure out how to do "incremental indexing" so I don't have to re-index everything after a small change.

## Rough Plan

| When         | What I'll work on                                       | Goal                                                          |
|--------------|---------------------------------------------------------|---------------------------------------------------------------|
| **This Week**  | **Stop the Pain:** Merging config & making a good README. | Get the project into a state where it's easy to work on again.  |
| **Next**       | **Clean Up:** Tidy the Rust/Python bridge, add some tests. | Make the code more stable and trustworthy.                     |
| **Later**      | **The Fun Stuff:** New features for detection & indexing. | Start building cool new capabilities on a solid foundation.   |

## Tracking Progress

#### Next Up:
1.  Start merging the two config files into one.
2.  Create a new `README.md` and start writing down the setup steps.

#### Done:
-   *Nothing yet!*

## Gut Check

#### What could go wrong?
-   The Rust/Python refactoring could break everything and take a long time to fix.
-   I might get bogged down in cleanup and never get to the fun feature work.
-   I might forget how a piece of code works if I don't add comments now.

#### What does "good enough" look like?
-   **Config:** One config file that's easy to understand.
-   **Docs:** I can clone the repo on a new machine and get it running in 10 minutes just by following the `README`.
-   **Testing:** I feel confident that the core logic works and won't break silently.
-   **Code:** I can come back to the project after a month and not be totally lost.