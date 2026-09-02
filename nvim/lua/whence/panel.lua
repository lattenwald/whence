local M = {}

function M.show(tree, ctx)
  M.last = { tree = tree, ctx = ctx }
end

return M
