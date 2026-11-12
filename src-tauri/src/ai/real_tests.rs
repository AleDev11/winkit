//! Pruebas contra las APIs de verdad. Gastan tokens: solo a mano.
//! `cargo test --lib ai::real_tests -- --ignored --nocapture --test-threads=1`

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap()
}

/// Claude con la key guardada en Ajustes (Haiku): los mismos casos que antes
/// del cambio a varios proveedores, para ver que el comportamiento no cambia.
#[test]
#[ignore]
fn claude_por_la_capa_nueva() {
    let s = Settings::default();
    assert_eq!(s.ai.for_tool("writing").model, "claude-haiku-5-5");
    let rt = rt();
    let deltas = AtomicUsize::new(0);
    let on_delta = |_: &str| {
        deltas.fetch_add(1, Ordering::Relaxed);
    };
    let casos = [
        ("correct", "Ola, mañana no podre ir a la reunion por que tengo medico.", None),
        ("translate", "Oye, ¿podemos mover la reunión al jueves por la tarde? Me ha surgido algo.", Some("Inglés")),
        ("translate", "Hey, can we move the meeting to Thursday afternoon? Something came up.", Some("Español")),
        ("understand", "ngl that meeting was lowkey a waste of time, let's just async it next week", Some("Español")),
        ("formal", "oye pásame el informe cuando puedas porfa", None),
    ];
    for (action, text, language) in casos {
        let req = RunRequest { action: action.into(), text: text.into(), language: language.map(String::from), instruction: None };
        let t = std::time::Instant::now();
        match rt.block_on(run(&s, &req, None, &on_delta)) {
            Ok(out) => println!("[{action}] {} ms\n  {text}\n  → {out}\n", t.elapsed().as_millis()),
            Err(f) => panic!("[{action}] error: {}", f.message),
        }
    }
    let t = std::time::Instant::now();
    let answer = rt.block_on(ask(&s, "¿Cuántos días tiene un año bisiesto?", &on_delta)).map_err(|f| f.message).unwrap();
    println!("[ask] {} ms\n  → {answer}\n", t.elapsed().as_millis());
    assert!(deltas.load(Ordering::Relaxed) > 5, "no ha llegado nada en streaming");
    let models = rt.block_on(models(&s, "anthropic")).map_err(|f| f.message).unwrap();
    println!("modelos de Anthropic: {:?}", models.iter().map(|m| &m.id).collect::<Vec<_>>());
    assert!(!models.is_empty());
    // Una key mala se rechaza con un aviso que abre Ajustes.
    let bad = rt.block_on(anthropic::test_key("sk-ant-mala")).unwrap_err();
    assert!(bad.open_settings, "{}", bad.message);
}

/// El resto de proveedores, solo si su key está en una variable de entorno.
#[test]
#[ignore]
fn otros_proveedores_si_hay_key() {
    let s = Settings::default();
    let rt = rt();
    for p in ["openai", "deepseek", "gemini", "openrouter"] {
        if settings::provider_key_from_env(p).is_none() {
            println!("[{p}] sin {} en el entorno: se salta", settings::key_env_var(p).unwrap());
            continue;
        }
        let list = rt.block_on(models(&s, p)).map_err(|f| f.message).unwrap();
        let m = list.iter().find(|m| m.recommended).or(list.first()).expect("sin modelos");
        let mref = ModelRef { provider: p.into(), model: m.id.clone() };
        let t = std::time::Instant::now();
        let req = RunRequest { action: "correct".into(), text: "Ola, mañana no podre ir a la reunion por que tengo medico.".into(), language: None, instruction: None };
        let user = prompts::user_message(&req, &[], None);
        let out = rt.block_on(chat(&s, &mref, prompts::SYSTEM, &user, &|_| {})).map_err(|f| f.message).unwrap();
        println!("[{p}] {} ({} modelos) {} ms → {out}", m.id, list.len(), t.elapsed().as_millis());
    }
    // La lista de OpenRouter es pública: se comprueba siempre el filtrado.
    let list = rt.block_on(models(&s, "openrouter")).map_err(|f| f.message).unwrap();
    println!("[openrouter] {} modelos de texto; recomendado: {:?}", list.len(), list.iter().find(|m| m.recommended).map(|m| (&m.id, &m.hint)));
    assert!(list.len() > 20);
}
