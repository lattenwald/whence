-- nvim --headless -u NONE -l scripts/record-fixture.lua <lang> <repo> <project> <file> <line> <col> <outdir>
-- line/col are 1-based (col is a byte column, like the cursor).
local lang, repo, project, file, line, col, outdir = arg[1], arg[2], arg[3], arg[4], arg[5], arg[6], arg[7]
line, col = tonumber(line), tonumber(col)

vim.opt.rtp:prepend(repo .. "/nvim")
vim.g.whence_bin = repo .. "/target/debug/whence"
vim.cmd("runtime plugin/whence.lua")
require("whence").setup({ root = project })

local servers = {
  rust = { name = "rust-analyzer", cmd = { "rust-analyzer" }, filetype = "rust" },
  go = { name = "gopls", cmd = { "gopls" }, filetype = "go" },
}
local s = servers[lang]
vim.cmd.cd(project)
vim.cmd.edit(file)
vim.bo.filetype = s.filetype
local client_id = vim.lsp.start({ name = s.name, cmd = s.cmd, root_dir = project }, { bufnr = 0 })
assert(client_id, "lsp did not start")

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

local done, err = false, nil
local record = require("whence.record")
record.begin(outdir, project, require("whence.util").cursor_target())
require("whence").trace(function(e) err = e; done = true end)
assert(vim.wait(60000, function() return done end, 50), "trace did not finish")
local hint = record.finish()
io.stdout:write((err and ("error: " .. vim.inspect(err)) or (hint or "recorded nothing")) .. "\n")
vim.cmd.qall({ bang = true })
