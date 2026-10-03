use drive_core::{
    catalog::{read_batch, FolderRequest},
    cli::Cli,
    operations::{OperationQueue, Spec, CANCELLED},
};
use std::{fs, os::unix::fs::PermissionsExt, time::Duration};

#[tokio::test]
async fn cancelling_a_slow_read_kills_its_process_and_releases_the_cli_gate() {
    let temp = tempfile::tempdir().unwrap();
    let binary = temp.path().join("fixture.py");
    fs::write(&binary, "#!/usr/bin/python3\nimport os,sys,time,pathlib\nif sys.argv[1] == 'slow':\n pathlib.Path(__file__).with_suffix('.pid').write_text(str(os.getpid()))\n time.sleep(2)\n pathlib.Path(__file__).with_suffix('.late').write_text('not killed')\nelse: print('ready')\n").unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    let cli = Cli {
        binary: binary.clone(),
    };
    let queue = OperationQueue::new(|| {});
    let q = queue.clone();
    let c = cli.clone();
    let task = tokio::spawn(async move {
        q.execute(Spec::new("browse", "slow", "").interactive(), |_| async {
            c.run(&["slow"], 90).await
        })
        .await
    });
    tokio::time::timeout(Duration::from_secs(3), async {
        while !binary.with_extension("pid").exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    queue.cancel(&queue.snapshot().items[0].id).unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap(),
        Err(CANCELLED.into())
    );
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), cli.run(&["fast"], 5))
            .await
            .unwrap()
            .unwrap()
            .trim(),
        "ready"
    );
    // Verify a post-cancellation side effect never happens. /proc is not
    // PID-namespace aligned in all sandboxes, so avoid probing it by PID.
    tokio::time::sleep(Duration::from_millis(2200)).await;
    assert!(
        !binary.with_extension("late").exists(),
        "cancelled CLI continued running"
    );
}

#[tokio::test]
async fn batch_validates_the_official_sdk_envelope_and_rejects_unverified_names() {
    let temp = tempfile::tempdir().unwrap();
    let binary = temp.path().join("fixture.py");
    fs::write(&binary, r#"#!/usr/bin/python3
import sys,json
requests=json.loads(sys.argv[3])
print(json.dumps([{'path':r['path'],'entries':[{'ok':True,'value':{'uid':'u1','name':{'ok':r['uid'] != 'bad','value':'Notes.pdf'},'type':'file','mediaType':'application/pdf','activeRevision':{'uid':'r1','storageSize':42}}}]} for r in requests]))
"#).unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    let cli = Cli { binary };
    let requests = [FolderRequest {
        path: "/my-files".into(),
        uid: None,
    }];
    let pages = read_batch(&cli, &requests).await.unwrap();
    assert_eq!(pages[0].1[0].path, "/my-files/Notes.pdf");
    assert_eq!(pages[0].1[0].uid, "u1");
    let invalid = [FolderRequest {
        path: "/my-files".into(),
        uid: Some("bad".into()),
    }];
    assert!(read_batch(&cli, &invalid).await.is_err());
}
