if vim.g.loaded_whence then
  return
end
vim.g.loaded_whence = true

vim.api.nvim_create_user_command("Whence", function()
  require("whence").trace()
end, { desc = "Trace the provenance of the identifier under the cursor" })

vim.keymap.set("n", "<Plug>(whence)", function()
  require("whence").trace()
end, { desc = "whence: trace under cursor" })

vim.cmd([[cnoreabbrev <expr> whence (getcmdtype() == ':' && getcmdline() ==# 'whence') ? 'Whence' : 'whence']])
