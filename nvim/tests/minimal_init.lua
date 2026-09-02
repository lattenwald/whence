vim.opt.rtp:prepend(vim.fn.getcwd() .. "/nvim")

local plenary = os.getenv("PLENARY_DIR")
if not plenary or plenary == "" then
  plenary = vim.fn.expand("~/.local/share/nvim/lazy/plenary.nvim")
end
vim.opt.rtp:prepend(plenary)
vim.cmd("runtime plugin/plenary.vim")

vim.g.whence_bin = vim.fn.getcwd() .. "/target/debug/whence"
