use crate::common::TestEnvironment;

/// Test to verify if jj status triggers rerere recording
#[test]
fn test_rerere_recording_with_status() {
    let test_env = TestEnvironment::default();
    test_env.add_config("rerere.enabled = true");
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    // Create a proper conflict
    work_dir.write_file("file.txt", "base\n");
    work_dir.run_jj(["commit", "-m", "base"]).success();

    // Get base commit ID
    let base_commit = work_dir
        .run_jj(["log", "-r", "@-", "--no-graph", "-T", "commit_id"])
        .success()
        .stdout
        .into_raw()
        .trim()
        .to_string();

    // Create two conflicting branches
    work_dir
        .run_jj(["new", &base_commit, "-m", "branch1"])
        .success();
    work_dir.write_file("file.txt", "branch1\n");
    work_dir
        .run_jj(["commit", "-m", "commit branch1"])
        .success();

    work_dir
        .run_jj(["new", &base_commit, "-m", "branch2"])
        .success();
    work_dir.write_file("file.txt", "branch2\n");
    work_dir
        .run_jj(["commit", "-m", "commit branch2"])
        .success();

    // Create a merge with conflict
    let merge_output = work_dir
        .run_jj([
            "new",
            "description(\"commit branch1\")",
            "description(\"commit branch2\")",
            "-m",
            "merge",
        ])
        .success();

    // Verify there's a conflict
    assert!(merge_output
        .stderr
        .raw()
        .contains("There are unresolved conflicts"));

    // Check the conflict markers
    let content = std::fs::read_to_string(work_dir.root().join("file.txt")).unwrap();
    assert!(content.contains("<<<<<<"));
    assert!(content.contains(">>>>>>"));

    // Resolve the conflict
    work_dir.write_file("file.txt", "resolved\n");

    // Run status - this should trigger snapshot and recording
    let status_output = work_dir.run_jj(["status"]).success();

    // Check if resolution was recorded
    let cache_dir = work_dir.root().join(".jj/repo/resolution_cache");

    // First, let's see what status says
    println!(
        "Status output after resolving:\n{}",
        status_output.to_string()
    );

    // The cache directory might not exist until we commit
    // Let's commit and check
    work_dir
        .run_jj(["commit", "-m", "resolved conflict"])
        .success();

    // Now check if cache exists
    if cache_dir.exists() {
        let entries: Vec<_> = std::fs::read_dir(&cache_dir).unwrap().collect();
        println!("Cache has {} entries", entries.len());
        assert!(
            !entries.is_empty(),
            "Resolution cache should have entries after commit"
        );
    } else {
        panic!("Resolution cache directory doesn't exist even after commit");
    }

    // Create another similar conflict to test if rerere applies
    // Use the same file path but different branches
    work_dir
        .run_jj(["new", &base_commit, "-m", "branch3"])
        .success();
    work_dir.write_file("file.txt", "branch1\n"); // Same content as original branch1
    work_dir
        .run_jj(["commit", "-m", "commit branch3"])
        .success();

    work_dir
        .run_jj(["new", &base_commit, "-m", "branch4"])
        .success();
    work_dir.write_file("file.txt", "branch2\n"); // Same content as original branch2
    work_dir
        .run_jj(["commit", "-m", "commit branch4"])
        .success();

    // Create another merge - rerere should apply
    let merge2_output = work_dir
        .run_jj([
            "new",
            "description(\"commit branch3\")",
            "description(\"commit branch4\")",
            "-m",
            "merge2",
        ])
        .success();

    // Check if rerere applied the cached resolution
    if merge2_output
        .stderr
        .raw()
        .contains("Applied 1 cached conflict resolution")
    {
        println!("SUCCESS: Rerere applied cached resolution!");
    } else {
        println!("Merge2 output:\n{}", merge2_output.to_string());
        panic!("Rerere did not apply cached resolution");
    }
}
