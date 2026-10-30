pub mod crypto;
pub mod data;
pub mod fs;
pub mod net;
pub mod sys;
pub mod text;

use ailang_core::{error::AilangResult, Value};

/// Dispatch a named function call with arguments and an optional input value.
/// This is the main entry point for the evaluator to call stdlib functions.
pub fn dispatch(
    name: &str,
    args: &[Value],
    input: Option<Value>,
    rt: &tokio::runtime::Handle,
) -> AilangResult<Vec<Value>> {
    match name {
        // ── fs ─────────────────────────────────────────────────────────────
        "read"          => fs::read(args, input),
        "write"         => fs::write(args, input),
        "glob"          => fs::glob(args, input),
        "walk"          => fs::walk(args, input),
        "cp"            => fs::cp(args, input),
        "mv"            => fs::mv(args, input),
        "rm"            => fs::rm(args, input),
        "exists"        => fs::exists(args, input),
        "stat"          => fs::stat(args, input),
        "diff"          => fs::diff(args, input),
        "patch"         => fs::patch_file(args, input),
        "replace_block" => fs::replace_block(args, input),

        // ── text ────────────────────────────────────────────────────────────
        "grep"    => text::grep(args, input),
        "sed"     => text::sed(args, input),
        "slice"   => text::slice(args, input),

        // ── data ────────────────────────────────────────────────────────────
        "filter"  => data::filter(args, input),
        "sort"    => data::sort(args, input),
        "group"   => data::group(args, input),
        "set"     => data::set_field(args, input),
        "assert"  => data::assert_val(args, input),

        // ── net ─────────────────────────────────────────────────────────────
        "net.get"      => rt.block_on(net::get(args, input)),
        "net.post"     => rt.block_on(net::post(args, input)),
        "net.download" => rt.block_on(net::download(args, input)),

        // ── sys ─────────────────────────────────────────────────────────────
        "exec"  => sys::exec(args, input),
        "env"   => sys::env(args, input),
        "pwd"   => sys::pwd(args, input),
        "cd"    => sys::cd(args, input),

        // ── crypto ──────────────────────────────────────────────────────────
        "base64.enc" => crypto::base64_enc(args, input),
        "base64.dec" => crypto::base64_dec(args, input),

        _ => Err(ailang_core::AilangError::UnknownFunction(name.to_string())),
    }
}
