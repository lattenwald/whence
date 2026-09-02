local root = vim.fs.dirname(vim.fs.dirname(debug.getinfo(1, "S").source:sub(2)))
vim.opt.runtimepath:append(root .. "/nvim")
dofile(root .. "/nvim/plugin/whence.lua")
