-- nvim --headless -u NONE -l scripts/record-fixture.lua <lang> <repo> <project> <file> <line> <col> <outdir>
-- line/col are 1-based (col is a byte column, like the cursor).
local lang, repo, project, file, line, col, outdir = arg[1], arg[2], arg[3], arg[4], arg[5], arg[6], arg[7]
line, col = tonumber(line), tonumber(col)

vim.opt.rtp:prepend(repo .. "/nvim")
vim.g.whence_bin = repo .. "/target/debug/whence"
vim.cmd("runtime plugin/whence.lua")
require("whence").setup({ root = project })

local servers = {
  -- the repo pins a toolchain without rust-src, and without std sources no method resolves
  rust = {
    name = "rust-analyzer",
    cmd = { "rust-analyzer" },
    filetype = "rust",
    ext = "rs",
    cmd_env = { RUSTUP_TOOLCHAIN = os.getenv("WHENCE_RUST_TOOLCHAIN") or "stable" },
  },
  go = { name = "gopls", cmd = { "gopls" }, filetype = "go", ext = "go" },
}
local s = servers[lang]
vim.cmd.cd(project)
vim.cmd.edit(file)
vim.bo.filetype = s.filetype
local client_id =
  vim.lsp.start({ name = s.name, cmd = s.cmd, cmd_env = s.cmd_env, root_dir = project }, { bufnr = 0 })
assert(client_id, "lsp did not start")
-- An editor's LSP config attaches to every buffer the host opens; stand in for it.
vim.api.nvim_create_autocmd("BufReadPost", {
  pattern = "*." .. s.ext,
  callback = function(ev)
    vim.lsp.buf_attach_client(ev.buf, client_id)
  end,
})

assert(vim.wait(20000, function() return #vim.lsp.get_clients({ bufnr = 0 }) > 0 end), "no client attached")
vim.api.nvim_win_set_cursor(0, { line, col - 1 })
local ready = vim.wait(120000, function()
  local r = vim.lsp.buf_request_sync(0, "textDocument/definition", vim.lsp.util.make_position_params(0, "utf-16"), 5000)
  for _, res in pairs(r or {}) do
    if res.result and (res.result.uri or (type(res.result) == "table" and #res.result > 0)) then return true end
  end
  return false
end, 1000)
assert(ready, "server never answered a definition at the target")
-- until indexing ends the server answers method and std lookups with nothing, silently
vim.wait(180000, function() return vim.lsp.status() == "" end, 200)
vim.wait(2000, function() return false end, 100)

local record = require("whence.record")
local hint, err
for attempt = 1, 5 do
  local done = false
  err = nil
  vim.fn.delete(outdir, "rf")
  record.begin(outdir, project, require("whence.util").cursor_target())
  require("whence").trace(function(e) err = e; done = true end)
  assert(vim.wait(60000, function() return done end, 50), "trace did not finish")
  hint = record.finish()
  -- rust-analyzer fails a request with "content modified" while it is still indexing
  if not err or attempt == 5 then break end
  vim.wait(5000, function() return false end, 100)
end
io.stdout:write((err and ("error: " .. vim.inspect(err)) or (hint or "recorded nothing")) .. "\n")
vim.cmd.qall({ bang = true })
