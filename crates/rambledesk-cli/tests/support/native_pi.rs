use super::*;

/// Opt-in: the supplied CLI must be an isolated copy under the OS temp directory.
/// It starts Pi RPC offline with empty HOME/agent state and never submits a model
/// prompt. A fixture extension invokes our production tool once at session start.
#[tokio::test]
#[ignore = "requires RAMBLEDESK_TEST_PI_CLI pointing to an isolated native Pi installation"]
async fn installed_native_pi_loads_managed_schema_and_creates_private_feedback()
-> anyhow::Result<()> {
    let native = std::path::PathBuf::from(
        std::env::var_os("RAMBLEDESK_TEST_PI_CLI").context("isolated Pi CLI")?,
    );
    anyhow::ensure!(
        native
            .canonicalize()?
            .starts_with(std::env::temp_dir().canonicalize()?),
        "use an isolated Pi copy under temp"
    );
    let fixture = Fixture::new().await?;
    let capability = fixture.provider.bind(&fixture.sessions[0]).await?;
    let extension = rambledesk_acp::pi_wrapper::install_managed_extension(
        &fixture._directory.path().join("pi-runtime"),
    )
    .await?;
    let marker = fixture._directory.path().join("native-result.json");
    let validator = native
        .parent()
        .context("Pi dist")?
        .parent()
        .context("Pi package")?
        .join("node_modules/@earendil-works/pi-ai/dist/utils/validation.js");
    let verifier = fixture._directory.path().join("verify-native.mjs");
    // The production registration still passes through the real Pi API. Capture
    // its definition only to invoke it deterministically without buying a model turn.
    let source = format!(
        r#"import {{writeFileSync}} from 'node:fs';
import {{pathToFileURL}} from 'node:url';
const managed=(await import(pathToFileURL({extension}).href)).default;
const validate=(await import(pathToFileURL({validator}).href)).validateToolArguments;
export default async function(pi) {{
  let request;
  await managed({{...pi,registerTool(tool){{if(tool.name==='request_feedback')request=tool;pi.registerTool(tool);}}}});
  pi.on('session_start',async(_event,ctx)=>{{
    try {{
      const args=validate(request,{{id:'native-fixture-call',name:'request_feedback',arguments:{{what_happened:'Native Pi RPC fixture',actions:[{{id:'review',instruction:'Review the native fixture'}}]}}}});
      const value=await request.execute('native-fixture-call',args,undefined,undefined,ctx);
      writeFileSync({marker},JSON.stringify({{names:pi.getAllTools().map(tool=>tool.name),result:value}}));
    }} catch {{writeFileSync({marker},JSON.stringify({{error:'Native managed tool failed'}}));}}
  }});
}}
"#,
        extension = serde_json::to_string(&extension)?,
        validator = serde_json::to_string(&validator)?,
        marker = serde_json::to_string(&marker)?
    );
    tokio::fs::write(&verifier, source).await?;
    let node = rambledesk_core::find_executable("node").context("Node")?;
    let home = fixture._directory.path().join("empty-home");
    tokio::fs::create_dir_all(&home).await?;
    let mut child = Command::new(env!("CARGO_BIN_EXE_rambledesk"))
        .args([
            "--mode",
            "rpc",
            "--no-themes",
            "--no-session",
            "--no-extensions",
            "--no-skills",
            "--no-prompt-templates",
            "--no-context-files",
            "--no-builtin-tools",
            "--offline",
        ])
        .current_dir(fixture._directory.path())
        .env(rambledesk_acp::pi_wrapper::WRAPPER_ENV, "1")
        .env(rambledesk_acp::pi_wrapper::COMMAND_ENV, node)
        .env(
            rambledesk_acp::pi_wrapper::ARGS_ENV,
            serde_json::to_string(&vec![native])?,
        )
        .env(rambledesk_acp::pi_wrapper::EXTENSION_ENV, verifier)
        .env(rambledesk_mcp::managed_stdio::URL_ENV, &capability.url)
        .env(
            rambledesk_mcp::managed_stdio::TOKEN_ENV,
            &capability.bearer_token,
        )
        .env("PI_CODING_AGENT_DIR", home.join("agent"))
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("LOCALAPPDATA", home.join("local"))
        .env("APPDATA", home.join("roaming"))
        .env("PI_OFFLINE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let mut input = child.stdin.take().context("stdin")?;
    let mut stdout = child.stdout.take().context("stdout")?;
    let mut stderr = child.stderr.take().context("stderr")?;
    let drain = tokio::spawn(async move {
        let _ = tokio::io::copy(&mut stdout, &mut tokio::io::sink()).await;
    });
    let errors = tokio::spawn(async move {
        let mut bytes = vec![];
        stderr.read_to_end(&mut bytes).await.unwrap();
        bytes
    });
    input
        .write_all(b"{\"id\":\"fixture-state\",\"type\":\"get_state\"}\n")
        .await?;
    tokio::time::timeout(Duration::from_secs(30), async {
        while !marker.exists() {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .context("native Pi session_start managed tool")?;
    let result: Value = serde_json::from_slice(&tokio::fs::read(&marker).await?)?;
    anyhow::ensure!(result.get("error").is_none(), "native tool fixture failed");
    let names = result["names"].as_array().context("native tool list")?;
    for name in ["request_feedback", "get_feedback", "recover_feedback"] {
        assert!(names.contains(&json!(name)));
    }
    assert!(!names.contains(&json!("request_ramble_feedback")));
    assert_eq!(result["result"]["details"]["status"], "waiting");
    let id = result["result"]["details"]["request_id"]
        .as_str()
        .context("request")?;
    assert_eq!(
        fixture
            .store
            .get_request(id)
            .await?
            .managed_session_id
            .as_deref(),
        Some("managed-a")
    );
    drop(input);
    tokio::time::timeout(Duration::from_secs(8), child.wait()).await??;
    drain.await?;
    let errors = errors.await?;
    assert!(!String::from_utf8_lossy(&errors).contains(&capability.bearer_token));
    fixture.server.shutdown().await?;
    fixture.store.close().await;
    Ok(())
}
