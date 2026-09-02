if vim.g.loaded_whence then
  return
end
vim.g.loaded_whence = true

vim.api.nvim_create_user_command("Whence", function()
  require("whence").trace()
end, { desc = "Trace the provenance of the identifier under the cursor" })

vim.api.nvim_create_user_command("WhenceRecord", function(cmd)
  local dir = vim.fn.fnamemodify(vim.fn.expand(cmd.args), ":p"):gsub("/$", "")
  require("whence.record").run(dir, require("whence").root())
end, { nargs = 1, complete = "dir", desc = "Record a replay fixture for the identifier under the cursor" })

vim.api.nvim_create_user_command("WhenceInstall", function()
  require("whence.install").install(function(err, exe)
    if err then
      vim.notify("whence: " .. err, vim.log.levels.ERROR)
    else
      vim.notify("whence: installed " .. exe)
    end
  end)
end, { desc = "Download the whence engine matching this plugin's version" })

vim.keymap.set("n", "<Plug>(whence)", function()
  require("whence").trace()
end, { desc = "whence: trace under cursor" })

vim.cmd([[cnoreabbrev <expr> whence (getcmdtype() == ':' && getcmdline() ==# 'whence') ? 'Whence' : 'whence']])
