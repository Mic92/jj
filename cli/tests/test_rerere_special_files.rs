// Copyright 2025 The Jujutsu Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::common::TestEnvironment;

/// Test rerere functionality with binary files
#[test]
fn test_rerere_binary_file_conflict() {
    let test_env = TestEnvironment::default();
    test_env.add_config("rerere.enabled = true");
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let repo_path = test_env.env_root().join("repo");

    // To test rerere with binary conflicts, we need a different setup
    // where the file exists in the base and is modified differently.
    // Let's create a proper binary conflict scenario

    // Create a base commit with a binary file
    test_env
        .run_jj_in(&repo_path, ["new", "root()", "-m", "base"])
        .success();
    std::fs::write(
        repo_path.join("image.png"),
        b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dBASE",
    )
    .unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "add image"])
        .success();

    // Create first branch that modifies the binary file
    test_env
        .run_jj_in(&repo_path, ["new", "@-", "-m", "side1"])
        .success();
    std::fs::write(
        repo_path.join("image.png"),
        b"\x89PNG\r\n\x1a\n\x00\x00\x00\x1aSIDE1",
    )
    .unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "modify image side1"])
        .success();

    // Create second branch from the base that modifies the same file differently
    test_env
        .run_jj_in(&repo_path, ["new", "@--", "-m", "side2"])
        .success();
    std::fs::write(
        repo_path.join("image.png"),
        b"\x89PNG\r\n\x1a\n\x00\x00\x00\x2bSIDE2",
    )
    .unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "modify image side2"])
        .success();

    // Get the commit ids for the two sides
    let side1_commit = test_env
        .run_jj_in(
            &repo_path,
            [
                "log",
                "--no-graph",
                "-r",
                "description(\"modify image side1\")",
                "-T",
                "commit_id",
            ],
        )
        .success()
        .stdout
        .into_raw()
        .trim()
        .to_string();
    let side2_commit = test_env
        .run_jj_in(
            &repo_path,
            [
                "log",
                "--no-graph",
                "-r",
                "description(\"modify image side2\")",
                "-T",
                "commit_id",
            ],
        )
        .success()
        .stdout
        .into_raw()
        .trim()
        .to_string();

    // Create a merge commit - should create a conflict
    let output = test_env
        .run_jj_in(
            &repo_path,
            ["new", &side1_commit, &side2_commit, "-m", "merge"],
        )
        .success();

    // First check if we got a conflict
    assert!(
        output.stderr.raw().contains("conflict"),
        "Expected conflict in first merge, got: {}",
        output.stderr.raw()
    );

    insta::assert_snapshot!(output.stderr, @r"
    Working copy  (@) now at: znkkpsqq cb1ed14a (conflict) (empty) merge
    Parent commit (@-)      : zsuskuln 2a95acc5 modify image side1
    Parent commit (@-)      : royxmykx a9fdb375 modify image side2
    Added 0 files, modified 1 files, removed 0 files
    Warning: There are unresolved conflicts at these paths:
    image.png    2-sided conflict
    [EOF]
    ");

    // Binary files should not be merged with conflict markers
    let content = std::fs::read(&repo_path.join("image.png")).unwrap();
    // Should still be binary content, not text with conflict markers
    assert!(content.starts_with(b"\x89PNG"));

    // Resolve the conflict by choosing a merged version
    std::fs::write(
        repo_path.join("image.png"),
        b"\x89PNG\r\n\x1a\n\x00\x00\x00\x3cMERGED",
    )
    .unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "resolved"])
        .success();

    // Create the same conflict in a different context
    test_env.run_jj_in(&repo_path, ["new", "root()"]).success();
    let base_id = test_env
        .run_jj_in(
            &repo_path,
            [
                "log",
                "--no-graph",
                "-r",
                "description(\"add image\")",
                "-T",
                "commit_id",
            ],
        )
        .success()
        .stdout
        .into_raw()
        .trim()
        .to_string();

    // Create new commits with the same changes to trigger rerere
    test_env
        .run_jj_in(&repo_path, ["new", &base_id, "-m", "base copy"])
        .success();
    test_env
        .run_jj_in(&repo_path, ["new", "@-", "-m", "side1 copy"])
        .success();
    std::fs::write(
        repo_path.join("image.png"),
        b"\x89PNG\r\n\x1a\n\x00\x00\x00\x1aSIDE1",
    )
    .unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "side1 copy commit"])
        .success();

    test_env
        .run_jj_in(&repo_path, ["new", "@--", "-m", "side2 copy"])
        .success();
    std::fs::write(
        repo_path.join("image.png"),
        b"\x89PNG\r\n\x1a\n\x00\x00\x00\x2bSIDE2",
    )
    .unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "side2 copy commit"])
        .success();

    // Get the commit ids for the two new sides
    let side1_copy_commit = test_env
        .run_jj_in(
            &repo_path,
            [
                "log",
                "--no-graph",
                "-r",
                "description(\"side1 copy commit\")",
                "-T",
                "commit_id",
            ],
        )
        .success()
        .stdout
        .into_raw()
        .trim()
        .to_string();
    let side2_copy_commit = test_env
        .run_jj_in(
            &repo_path,
            [
                "log",
                "--no-graph",
                "-r",
                "description(\"side2 copy commit\")",
                "-T",
                "commit_id",
            ],
        )
        .success()
        .stdout
        .into_raw()
        .trim()
        .to_string();

    // Create another merge - rerere should apply the resolution
    let output = test_env
        .run_jj_in(
            &repo_path,
            [
                "new",
                &side1_copy_commit,
                &side2_copy_commit,
                "-m",
                "merge2",
            ],
        )
        .success();

    // Rerere should have applied the cached resolution
    assert!(output
        .stderr
        .raw()
        .contains("Applied 1 cached conflict resolutions"));

    // The conflict should be automatically resolved
    assert!(!output
        .stderr
        .raw()
        .contains("Warning: There are unresolved conflicts"));

    // The file should have the cached resolution
    let content = std::fs::read(&repo_path.join("image.png")).unwrap();
    assert_eq!(content, b"\x89PNG\r\n\x1a\n\x00\x00\x00\x3cMERGED");
}

/// Test rerere functionality with executable bit conflicts
#[test]
#[cfg(unix)]
fn test_rerere_executable_bit_conflict() {
    let test_env = TestEnvironment::default();
    test_env.add_config("rerere.enabled = true");
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let repo_path = test_env.env_root().join("repo");

    // Create initial script file
    test_env
        .run_jj_in(&repo_path, ["new", "root()", "-m", "base"])
        .success();
    std::fs::write(repo_path.join("script.sh"), "#!/bin/bash\necho 'Hello'\n").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "add script"])
        .success();

    // Create diverging changes to the executable bit
    test_env
        .run_jj_in(&repo_path, ["new", "@-", "-m", "side1"])
        .success();
    std::fs::write(
        repo_path.join("script.sh"),
        "#!/bin/bash\necho 'Hello World'\n",
    )
    .unwrap();
    test_env
        .run_jj_in(&repo_path, ["file", "chmod", "x", "script.sh"])
        .success();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "make script executable"])
        .success();

    test_env
        .run_jj_in(&repo_path, ["new", "@--", "-m", "side2"])
        .success();
    std::fs::write(
        repo_path.join("script.sh"),
        "#!/bin/bash\necho 'Hello Universe'\n",
    )
    .unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "modify script content"])
        .success();

    // Get the commit ids for the two sides
    let side1_commit = test_env
        .run_jj_in(
            &repo_path,
            [
                "log",
                "--no-graph",
                "-r",
                "description(\"make script executable\")",
                "-T",
                "commit_id",
            ],
        )
        .success()
        .stdout
        .into_raw()
        .trim()
        .to_string();
    let side2_commit = test_env
        .run_jj_in(
            &repo_path,
            [
                "log",
                "--no-graph",
                "-r",
                "description(\"modify script content\")",
                "-T",
                "commit_id",
            ],
        )
        .success()
        .stdout
        .into_raw()
        .trim()
        .to_string();

    // Create a merge commit - should create a conflict
    let output = test_env
        .run_jj_in(
            &repo_path,
            ["new", &side1_commit, &side2_commit, "-m", "merge"],
        )
        .success();
    assert!(output
        .stderr
        .raw()
        .contains("There are unresolved conflicts"));

    // Resolve the conflict, keeping executable bit and merging content
    std::fs::write(
        repo_path.join("script.sh"),
        "#!/bin/bash\necho 'Hello World Universe'\n",
    )
    .unwrap();
    test_env
        .run_jj_in(&repo_path, ["file", "chmod", "x", "script.sh"])
        .success();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "resolved conflict"])
        .success();

    // Create the same conflict in a different context with new files
    test_env.run_jj_in(&repo_path, ["new", "root()"]).success();
    test_env
        .run_jj_in(&repo_path, ["new", "-m", "base2"])
        .success();
    std::fs::write(repo_path.join("script2.sh"), "#!/bin/bash\necho 'Hello'\n").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "add script2"])
        .success();

    // Create side1 with executable bit
    test_env
        .run_jj_in(&repo_path, ["new", "@-", "-m", "side1-2"])
        .success();
    std::fs::write(
        repo_path.join("script2.sh"),
        "#!/bin/bash\necho 'Hello World'\n",
    )
    .unwrap();
    test_env
        .run_jj_in(&repo_path, ["file", "chmod", "x", "script2.sh"])
        .success();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "make script2 executable"])
        .success();

    // Create side2 with content change
    test_env
        .run_jj_in(&repo_path, ["new", "@--", "-m", "side2-2"])
        .success();
    std::fs::write(
        repo_path.join("script2.sh"),
        "#!/bin/bash\necho 'Hello Universe'\n",
    )
    .unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "modify script2 content"])
        .success();

    // Get the commit ids for the two sides
    let side1_id = test_env
        .run_jj_in(
            &repo_path,
            [
                "log",
                "--no-graph",
                "-r",
                "description(\"make script2 executable\")",
                "-T",
                "commit_id",
            ],
        )
        .success()
        .stdout
        .into_raw()
        .trim()
        .to_string();
    let side2_id = test_env
        .run_jj_in(
            &repo_path,
            [
                "log",
                "--no-graph",
                "-r",
                "description(\"modify script2 content\")",
                "-T",
                "commit_id",
            ],
        )
        .success()
        .stdout
        .into_raw()
        .trim()
        .to_string();

    // Create a merge - rerere should apply the cached resolution
    let output = test_env
        .run_jj_in(&repo_path, ["new", &side1_id, &side2_id, "-m", "merge2"])
        .success();

    // Check if rerere applied the resolution
    assert!(output
        .stderr
        .raw()
        .contains("Applied 1 cached conflict resolutions"));
    assert!(!output
        .stderr
        .raw()
        .contains("Warning: There are unresolved conflicts"));

    // The file should have the cached resolution
    let script_content = std::fs::read_to_string(&repo_path.join("script2.sh")).unwrap();
    assert_eq!(script_content, "#!/bin/bash\necho 'Hello World Universe'\n");

    // Check executable bit
    use std::os::unix::fs::PermissionsExt;
    let metadata = std::fs::metadata(&repo_path.join("script2.sh")).unwrap();
    let permissions = metadata.permissions();
    assert!(permissions.mode() & 0o111 != 0);
}

/// Test rerere functionality with empty file conflicts
#[test]
fn test_rerere_empty_file_conflict() {
    let test_env = TestEnvironment::default();
    test_env.add_config("rerere.enabled = true");
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let repo_path = test_env.env_root().join("repo");

    // Create initial file
    test_env
        .run_jj_in(&repo_path, ["new", "root()", "-m", "base"])
        .success();
    std::fs::write(repo_path.join("file.txt"), "content").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "add file"])
        .success();

    // Create diverging changes - one empties the file
    test_env
        .run_jj_in(&repo_path, ["new", "@-", "-m", "side1"])
        .success();
    std::fs::write(repo_path.join("file.txt"), "").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "empty file"])
        .success();

    test_env
        .run_jj_in(&repo_path, ["new", "@--", "-m", "side2"])
        .success();
    std::fs::write(repo_path.join("file.txt"), "new content").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "modify file"])
        .success();

    // Get the commit ids for the two sides
    let side1_commit = test_env
        .run_jj_in(
            &repo_path,
            [
                "log",
                "--no-graph",
                "-r",
                "description(\"empty file\")",
                "-T",
                "commit_id",
            ],
        )
        .success()
        .stdout
        .into_raw()
        .trim()
        .to_string();
    let side2_commit = test_env
        .run_jj_in(
            &repo_path,
            [
                "log",
                "--no-graph",
                "-r",
                "description(\"modify file\")",
                "-T",
                "commit_id",
            ],
        )
        .success()
        .stdout
        .into_raw()
        .trim()
        .to_string();

    // Create a merge commit - should create a conflict
    let output = test_env
        .run_jj_in(
            &repo_path,
            ["new", &side1_commit, &side2_commit, "-m", "merge"],
        )
        .success();
    assert!(output
        .stderr
        .raw()
        .contains("There are unresolved conflicts"));

    // Resolve the conflict by keeping it empty
    std::fs::write(repo_path.join("file.txt"), "").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "resolved merge"])
        .success();

    // Create the same conflict in a different context by recreating the same
    // divergence
    test_env.run_jj_in(&repo_path, ["new", "root()"]).success();

    // Create a new base file
    test_env
        .run_jj_in(&repo_path, ["new", "-m", "base2"])
        .success();
    std::fs::write(repo_path.join("file2.txt"), "content").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "add file2"])
        .success();

    // Create the same diverging changes on a different file
    test_env
        .run_jj_in(&repo_path, ["new", "@-", "-m", "side1b"])
        .success();
    std::fs::write(repo_path.join("file2.txt"), "").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "empty file2"])
        .success();

    test_env
        .run_jj_in(&repo_path, ["new", "@--", "-m", "side2b"])
        .success();
    std::fs::write(repo_path.join("file2.txt"), "new content").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "modify file2"])
        .success();

    // Get the commit ids for the new sides
    let side1b_commit = test_env
        .run_jj_in(
            &repo_path,
            [
                "log",
                "--no-graph",
                "-r",
                "description(\"empty file2\")",
                "-T",
                "commit_id",
            ],
        )
        .success()
        .stdout
        .into_raw()
        .trim()
        .to_string();
    let side2b_commit = test_env
        .run_jj_in(
            &repo_path,
            [
                "log",
                "--no-graph",
                "-r",
                "description(\"modify file2\")",
                "-T",
                "commit_id",
            ],
        )
        .success()
        .stdout
        .into_raw()
        .trim()
        .to_string();

    // Create a new merge with the same logical conflict
    let output = test_env
        .run_jj_in(
            &repo_path,
            ["new", &side1b_commit, &side2b_commit, "-m", "merge2"],
        )
        .success();

    // Check if rerere applied the resolution automatically
    assert!(output
        .stderr
        .raw()
        .contains("Applied 1 cached conflict resolutions"));

    // Verify the file has the cached resolution
    let content = std::fs::read_to_string(&repo_path.join("file2.txt")).unwrap();
    assert_eq!(content, "");
}

/// Test rerere functionality with mixed text/binary conflicts
#[test]
fn test_rerere_mixed_text_binary_conflict() {
    let test_env = TestEnvironment::default();
    test_env.add_config("rerere.enabled = true");
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let repo_path = test_env.env_root().join("repo");

    // Create initial text file
    test_env
        .run_jj_in(&repo_path, ["new", "root()", "-m", "base"])
        .success();
    std::fs::write(repo_path.join("data.txt"), "text content").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "add text file"])
        .success();

    // One side keeps it as text, other converts to binary
    test_env
        .run_jj_in(&repo_path, ["new", "@-", "-m", "side1"])
        .success();
    std::fs::write(repo_path.join("data.txt"), "updated text content").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "update text"])
        .success();

    test_env
        .run_jj_in(&repo_path, ["new", "@--", "-m", "side2"])
        .success();
    std::fs::write(repo_path.join("data.txt"), b"\x00\x01\x02binary\x03\x04").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "convert to binary"])
        .success();

    // Get commit IDs for the merge - we need the two divergent commits
    let output = test_env
        .run_jj_in(
            &repo_path,
            [
                "log",
                "-r",
                "description(\"update text\")",
                "-T",
                "commit_id",
                "--no-graph",
            ],
        )
        .success();
    let side1_commit = output.stdout.raw().trim();

    let output = test_env
        .run_jj_in(
            &repo_path,
            [
                "log",
                "-r",
                "description(\"convert to binary\")",
                "-T",
                "commit_id",
                "--no-graph",
            ],
        )
        .success();
    let side2_commit = output.stdout.raw().trim();

    // Create a merge commit - should create a conflict
    let output = test_env
        .run_jj_in(
            &repo_path,
            ["new", side1_commit, side2_commit, "-m", "merge"],
        )
        .success();
    assert!(output
        .stderr
        .raw()
        .contains("There are unresolved conflicts"));

    // Resolve by choosing text version
    std::fs::write(repo_path.join("data.txt"), "resolved text content").unwrap();

    // Commit the resolution to trigger rerere recording
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "resolved conflict"])
        .success();

    // Create the same conflict on different files
    test_env
        .run_jj_in(&repo_path, ["new", "root()", "-m", "new base"])
        .success();
    std::fs::write(repo_path.join("data2.txt"), "text content").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "add text file 2"])
        .success();

    // Recreate the same logical conflict
    test_env
        .run_jj_in(&repo_path, ["new", "@-", "-m", "new side1"])
        .success();
    std::fs::write(repo_path.join("data2.txt"), "updated text content").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "update text 2"])
        .success();

    test_env
        .run_jj_in(&repo_path, ["new", "@--", "-m", "new side2"])
        .success();
    std::fs::write(repo_path.join("data2.txt"), b"\x00\x01\x02binary\x03\x04").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "convert to binary 2"])
        .success();

    // Get new commit IDs
    let output = test_env
        .run_jj_in(
            &repo_path,
            [
                "log",
                "-r",
                "description(\"update text 2\")",
                "-T",
                "commit_id",
                "--no-graph",
            ],
        )
        .success();
    let new_side1_commit = output.stdout.raw().trim();

    let output = test_env
        .run_jj_in(
            &repo_path,
            [
                "log",
                "-r",
                "description(\"convert to binary 2\")",
                "-T",
                "commit_id",
                "--no-graph",
            ],
        )
        .success();
    let new_side2_commit = output.stdout.raw().trim();

    // Create merge - rerere should apply cached resolution
    let output = test_env
        .run_jj_in(
            &repo_path,
            ["new", new_side1_commit, new_side2_commit, "-m", "merge2"],
        )
        .success();

    // Check if rerere applied the resolution
    assert!(output
        .stderr
        .raw()
        .contains("Applied 1 cached conflict resolutions"));

    // Verify the file has the cached resolution
    let content = std::fs::read_to_string(&repo_path.join("data2.txt")).unwrap();
    assert_eq!(content, "resolved text content");
}
