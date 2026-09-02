local M = {}

function M.rel(file, root)
  if root and root ~= "" then
    local rel = vim.fs.relpath(root, file)
    if rel then
      return rel
    end
  end
  return file
end

-- Same scheme as engine/src/host_replay.rs; the fixtures are shared.
function M.fixture_key(root, method, params)
  local k = ("%s:%d:%d"):format(M.rel(params.file, root), params.line, params.col)
  if method == "host/references" then
    k = k .. (params.includeDeclaration and "|decl" or "|nodecl")
  end
  return k
end

-- str_utfindex/str_byteindex error past the end of the line, hence the fallbacks.
function M.utf16_col(text, byte_col)
  local ok, col = pcall(vim.str_utfindex, text, "utf-16", byte_col)
  return ok and col or byte_col
end

function M.byte_col(text, utf16_col)
  local ok, col = pcall(vim.str_byteindex, text, "utf-16", utf16_col)
  return ok and col or #text
end

function M.cursor_target()
  local file = vim.api.nvim_buf_get_name(0)
  if file == "" then
    return nil
  end
  local cursor = vim.api.nvim_win_get_cursor(0)
  local line = cursor[1] - 1
  local text = vim.api.nvim_buf_get_lines(0, line, line + 1, false)[1] or ""
  return { file = file, line = line, col = M.utf16_col(text, cursor[2]) }
end

return M
