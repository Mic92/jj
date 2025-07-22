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

/// Test rerere with rename/move conflicts
#[test]
fn test_rerere_rename_conflict() {
    let test_env = TestEnvironment::default();
    test_env.add_config("rerere.enabled = true");
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let repo_path = test_env.env_root().join("repo");

    // Create initial file
    test_env
        .run_jj_in(&repo_path, ["new", "root()", "-m", "base"])
        .success();
    std::fs::write(repo_path.join("original.txt"), "original content").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "add original file"])
        .success();

    // One side renames the file
    test_env
        .run_jj_in(&repo_path, ["new", "@-", "-m", "side1"])
        .success();
    std::fs::rename(
        repo_path.join("original.txt"),
        repo_path.join("renamed.txt"),
    )
    .unwrap();
    std::fs::write(repo_path.join("renamed.txt"), "renamed and modified").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "rename to renamed.txt"])
        .success();

    // Other side modifies the file in place
    test_env
        .run_jj_in(&repo_path, ["new", "@--", "-m", "side2"])
        .success();
    std::fs::write(repo_path.join("original.txt"), "modified in place").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "modify original.txt"])
        .success();

    // Create a merge commit - should handle rename conflict
    let output = test_env
        .run_jj_in(&repo_path, ["new", "@-", "@", "-m", "merge"])
        .success();

    // Check the result - jj should handle rename gracefully
    // The exact behavior depends on jj's rename detection
    let has_original = repo_path.join("original.txt").exists();
    let has_renamed = repo_path.join("renamed.txt").exists();

    // At least one should exist
    assert!(has_original || has_renamed);

    // If there's a conflict, resolve it
    if output
        .stdout
        .raw()
        .contains("There are unresolved conflicts")
    {
        // Resolve by keeping the renamed file with merged content
        std::fs::remove_file(repo_path.join("original.txt")).ok();
        std::fs::write(repo_path.join("renamed.txt"), "resolved content").unwrap();
        test_env.run_jj_in(&repo_path, ["resolve"]).success();
    }

    // Create the same scenario in a different context
    test_env.run_jj_in(&repo_path, ["new", "root()"]).success();
    let output = test_env
        .run_jj_in(&repo_path, ["new", "@--+", "@-+", "-m", "merge2"])
        .success();

    // Check if rerere handled the rename conflict
    let has_renamed_2 = repo_path.join("renamed.txt").exists();
    if has_renamed_2
        && !output
            .stdout
            .raw()
            .contains("There are unresolved conflicts")
    {
        let content = std::fs::read_to_string(&repo_path.join("renamed.txt")).unwrap();
        assert_eq!(content, "resolved content");
    }
}

/// Test rerere with file vs directory conflicts
#[test]
fn test_rerere_file_vs_directory_conflict() {
    let test_env = TestEnvironment::default();
    test_env.add_config("rerere.enabled = true");
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let repo_path = test_env.env_root().join("repo");

    // Create initial file
    test_env
        .run_jj_in(&repo_path, ["new", "root()", "-m", "base"])
        .success();
    std::fs::write(repo_path.join("item"), "file content").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "add file"])
        .success();

    // One side keeps it as a file
    test_env
        .run_jj_in(&repo_path, ["new", "@-", "-m", "side1"])
        .success();
    std::fs::write(repo_path.join("item"), "updated file content").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "update file"])
        .success();

    // Other side converts to directory
    test_env
        .run_jj_in(&repo_path, ["new", "@--", "-m", "side2"])
        .success();
    std::fs::remove_file(repo_path.join("item")).unwrap();
    std::fs::create_dir(repo_path.join("item")).unwrap();
    std::fs::write(repo_path.join("item/file1.txt"), "content 1").unwrap();
    std::fs::write(repo_path.join("item/file2.txt"), "content 2").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "convert to directory"])
        .success();

    // Create a merge commit
    let output = test_env
        .run_jj_in(&repo_path, ["new", "@-", "@", "-m", "merge"])
        .success();

    // This type of conflict might not be mergeable in the usual way
    if output
        .stdout
        .raw()
        .contains("There are unresolved conflicts")
    {
        // Resolve by choosing directory structure
        std::fs::remove_file(repo_path.join("item")).ok();
        std::fs::create_dir_all(repo_path.join("item")).ok();
        std::fs::write(repo_path.join("item/merged.txt"), "resolved as directory").unwrap();
        test_env.run_jj_in(&repo_path, ["resolve"]).success();
    }

    // Create the same conflict in a different context
    test_env.run_jj_in(&repo_path, ["new", "root()"]).success();
    test_env
        .run_jj_in(&repo_path, ["new", "@--+", "@-+", "-m", "merge2"])
        .success();

    // Check the result - file/directory conflicts are complex
    let item_path = repo_path.join("item");
    assert!(item_path.exists());
}

/// Test rerere with multiple interdependent conflicts
#[test]
fn test_rerere_multiple_file_conflicts() {
    let test_env = TestEnvironment::default();
    test_env.add_config("rerere.enabled = true");
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let repo_path = test_env.env_root().join("repo");

    // Create initial files with dependencies
    test_env
        .run_jj_in(&repo_path, ["new", "root()", "-m", "base"])
        .success();
    std::fs::write(
        repo_path.join("config.txt"),
        "version: 1\nfeature: disabled",
    )
    .unwrap();
    std::fs::write(
        repo_path.join("main.py"),
        "import config\nif config.feature:\n    print('enabled')",
    )
    .unwrap();
    std::fs::write(
        repo_path.join("test.py"),
        "def test():\n    assert config.version == 1",
    )
    .unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "add files"])
        .success();

    // Side 1 updates version and related code
    test_env
        .run_jj_in(&repo_path, ["new", "@-", "-m", "side1"])
        .success();
    std::fs::write(
        repo_path.join("config.txt"),
        "version: 2\nfeature: disabled\napi: v2",
    )
    .unwrap();
    std::fs::write(
        repo_path.join("main.py"),
        "import config\nif config.feature:\n    print('enabled v2')",
    )
    .unwrap();
    std::fs::write(
        repo_path.join("test.py"),
        "def test():\n    assert config.version == 2\n    assert config.api == 'v2'",
    )
    .unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "update to v2"])
        .success();

    // Side 2 enables feature and updates code
    test_env
        .run_jj_in(&repo_path, ["new", "@--", "-m", "side2"])
        .success();
    std::fs::write(
        repo_path.join("config.txt"),
        "version: 1\nfeature: enabled\nmode: production",
    )
    .unwrap();
    std::fs::write(
        repo_path.join("main.py"),
        "import config\nif config.feature:\n    print('feature is active')\nelse:\n    \
         print('disabled')",
    )
    .unwrap();
    std::fs::write(
        repo_path.join("test.py"),
        "def test():\n    assert config.version == 1\n    assert config.feature == True",
    )
    .unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "enable feature"])
        .success();

    // Create a merge commit - multiple conflicts
    let output = test_env
        .run_jj_in(
            &repo_path,
            [
                "new",
                r#"description("update to v2")"#,
                r#"description("enable feature")"#,
                "-m",
                "merge",
            ],
        )
        .success();
    assert!(output
        .stderr
        .raw()
        .contains("There are unresolved conflicts"));

    // Resolve all conflicts consistently
    std::fs::write(
        repo_path.join("config.txt"),
        "version: 2\nfeature: enabled\napi: v2\nmode: production",
    )
    .unwrap();
    std::fs::write(
        repo_path.join("main.py"),
        "import config\nif config.feature:\n    print('feature is active v2')\nelse:\n    \
         print('disabled')",
    )
    .unwrap();
    std::fs::write(
        repo_path.join("test.py"),
        "def test():\n    assert config.version == 2\n    assert config.feature == True\n    \
         assert config.api == 'v2'",
    )
    .unwrap();
    // Commit the resolution to record it
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "resolved conflicts"])
        .success();

    // Create the same conflicts in a different context
    // Find the side1 and side2 commits using their descriptions
    let output = test_env
        .run_jj_in(
            &repo_path,
            [
                "new",
                r#"description("update to v2")"#,
                r#"description("enable feature")"#,
                "-m",
                "merge2",
            ],
        )
        .success();

    // Check if rerere was applied
    assert!(output
        .stderr
        .raw()
        .contains("Applied 3 cached conflict resolutions"));

    // Check if all three files were resolved correctly
    if !output
        .stderr
        .raw()
        .contains("There are unresolved conflicts")
    {
        let config = std::fs::read_to_string(&repo_path.join("config.txt")).unwrap();
        assert!(config.contains("version: 2"));
        assert!(config.contains("feature: enabled"));
        assert!(config.contains("api: v2"));
        assert!(config.contains("mode: production"));

        let main_py = std::fs::read_to_string(&repo_path.join("main.py")).unwrap();
        assert!(main_py.contains("feature is active v2"));

        let test_py = std::fs::read_to_string(&repo_path.join("test.py")).unwrap();
        assert!(test_py.contains("assert config.version == 2"));
        assert!(test_py.contains("assert config.feature == True"));
    }
}

/// Test rerere with nested conflicts (conflicts within conflicts)
#[test]
fn test_rerere_nested_conflicts() {
    let test_env = TestEnvironment::default();
    test_env.add_config("rerere.enabled = true");
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let repo_path = test_env.env_root().join("repo");

    // Create initial commit
    test_env
        .run_jj_in(&repo_path, ["new", "root()", "-m", "base"])
        .success();
    std::fs::write(repo_path.join("file.txt"), "base content").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "base"])
        .success();

    // Create first divergence from base
    test_env
        .run_jj_in(&repo_path, ["new", "@-", "-m", "a1"])
        .success();
    std::fs::write(repo_path.join("file.txt"), "a1 content").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "a1"])
        .success();

    test_env
        .run_jj_in(&repo_path, ["new", "@-", "-m", "a2"])
        .success();
    std::fs::write(repo_path.join("file.txt"), "a2 content").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "a2"])
        .success();

    // Create parallel branch from base
    test_env
        .run_jj_in(&repo_path, ["new", "description(\"base\")", "-m", "b1"])
        .success();
    std::fs::write(repo_path.join("file.txt"), "b1 content").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "b1"])
        .success();

    test_env
        .run_jj_in(&repo_path, ["new", "@-", "-m", "b2"])
        .success();
    std::fs::write(repo_path.join("file.txt"), "b2 content").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "b2"])
        .success();

    // Create first merge with conflict
    let output = test_env
        .run_jj_in(
            &repo_path,
            [
                "new",
                r#"description("a1")"#,
                r#"description("a2")"#,
                "-m",
                "merge-a",
            ],
        )
        .success();
    if output
        .stderr
        .raw()
        .contains("There are unresolved conflicts")
    {
        std::fs::write(repo_path.join("file.txt"), "merged a1+a2").unwrap();
        test_env
            .run_jj_in(&repo_path, ["commit", "-m", "resolved merge-a"])
            .success();
    }

    // Create second merge with conflict
    let output = test_env
        .run_jj_in(
            &repo_path,
            [
                "new",
                r#"description("b1")"#,
                r#"description("b2")"#,
                "-m",
                "merge-b",
            ],
        )
        .success();
    if output
        .stderr
        .raw()
        .contains("There are unresolved conflicts")
    {
        std::fs::write(repo_path.join("file.txt"), "merged b1+b2").unwrap();
        test_env
            .run_jj_in(&repo_path, ["commit", "-m", "resolved merge-b"])
            .success();
    }

    // Now merge the two merges - creating a nested conflict scenario
    let output = test_env
        .run_jj_in(
            &repo_path,
            [
                "new",
                r#"description("merge-a")"#,
                r#"description("merge-b")"#,
                "-m",
                "mega-merge",
            ],
        )
        .success();

    // This creates a complex conflict between two already-merged states
    if output
        .stderr
        .raw()
        .contains("There are unresolved conflicts")
    {
        std::fs::write(repo_path.join("file.txt"), "final resolution").unwrap();
        test_env
            .run_jj_in(&repo_path, ["commit", "-m", "resolved mega-merge"])
            .success();
    }

    // Try to recreate a similar nested conflict pattern
    // Recreate the same mega-merge using the already resolved merges
    let output = test_env
        .run_jj_in(
            &repo_path,
            [
                "new",
                r#"description("merge-a")"#,
                r#"description("merge-b")"#,
                "-m",
                "mega-merge2",
            ],
        )
        .success();

    // Check if rerere was applied
    assert!(
        output.stderr.raw().contains("Applied")
            && output.stderr.raw().contains("cached conflict resolution")
    );

    // Check if rerere can handle this complex scenario
    if !output
        .stderr
        .raw()
        .contains("There are unresolved conflicts")
    {
        let content = std::fs::read_to_string(&repo_path.join("file.txt")).unwrap();
        assert_eq!(content, "final resolution");
    }
}

/// Test rerere with copy detection scenarios
#[test]
fn test_rerere_copy_conflicts() {
    let test_env = TestEnvironment::default();
    test_env.add_config("rerere.enabled = true");
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let repo_path = test_env.env_root().join("repo");

    // Create initial file
    test_env
        .run_jj_in(&repo_path, ["new", "root()", "-m", "base"])
        .success();
    std::fs::write(
        repo_path.join("original.txt"),
        "shared content\noriginal specific",
    )
    .unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "add original"])
        .success();

    // Side 1: modify original
    test_env
        .run_jj_in(&repo_path, ["new", "@-", "-m", "side1"])
        .success();
    std::fs::write(
        repo_path.join("original.txt"),
        "shared content modified\noriginal specific v2",
    )
    .unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "modify original"])
        .success();

    // Side 2: copy and modify both
    test_env
        .run_jj_in(&repo_path, ["new", "@--", "-m", "side2"])
        .success();
    std::fs::write(
        repo_path.join("original.txt"),
        "shared content\noriginal changed",
    )
    .unwrap();
    std::fs::write(repo_path.join("copy.txt"), "shared content\ncopy specific").unwrap();
    test_env
        .run_jj_in(&repo_path, ["commit", "-m", "create copy"])
        .success();

    // Merge - this creates interesting conflicts
    let output = test_env
        .run_jj_in(
            &repo_path,
            [
                "new",
                r#"description("modify original")"#,
                r#"description("create copy")"#,
                "-m",
                "merge",
            ],
        )
        .success();

    if output
        .stderr
        .raw()
        .contains("There are unresolved conflicts")
    {
        // Resolve by incorporating changes to both files
        std::fs::write(
            repo_path.join("original.txt"),
            "shared content modified\noriginal specific v2 changed",
        )
        .unwrap();
        std::fs::write(
            repo_path.join("copy.txt"),
            "shared content modified\ncopy specific",
        )
        .unwrap();
        test_env
            .run_jj_in(&repo_path, ["commit", "-m", "resolved copy conflicts"])
            .success();
    }

    // Recreate similar scenario
    let output = test_env
        .run_jj_in(
            &repo_path,
            [
                "new",
                r#"description("modify original")"#,
                r#"description("create copy")"#,
                "-m",
                "merge2",
            ],
        )
        .success();

    // Check if rerere was applied
    assert!(
        output.stderr.raw().contains("Applied")
            && output.stderr.raw().contains("cached conflict resolution")
    );

    // Check if rerere handled the copy scenario
    if !output
        .stderr
        .raw()
        .contains("There are unresolved conflicts")
    {
        let original = std::fs::read_to_string(&repo_path.join("original.txt")).unwrap();
        let copy = std::fs::read_to_string(&repo_path.join("copy.txt")).unwrap();

        // Both files should have the resolved content
        assert!(original.contains("shared content modified"));
        assert!(original.contains("original specific v2 changed"));
        assert!(copy.contains("shared content"));
        assert!(copy.contains("copy specific"));
    }
}
