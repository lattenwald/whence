local M = {}

local TIMEOUT = 5000
local METHOD_NOT_FOUND = -32601
local INTERNAL_ERROR = -32603

local HIGHLIGHT_KIND = { "text", "read", "write" }

function M.bufnr_for(file)
  local b = vim.fn.bufadd(file)
  vim.fn.bufload(b)
  vim.bo[b].buflisted = false
  vim.wait(2000, function()
    return #vim.lsp.get_clients({ bufnr = b }) > 0
  end)
  return b
end

local function line_of(bufnr, line)
  return vim.api.nvim_buf_get_lines(bufnr, line, line + 1, false)[1] or ""
end

local function from_utf16(text, col, encoding)
  if encoding == "utf-16" then
    return col
  end
  local ok, byte = pcall(vim.str_byteindex, text, "utf-16", col)
  if not ok then
    return col
  end
  if encoding == "utf-8" then
    return byte
  end
  local ok2, idx = pcall(vim.str_utfindex, text, encoding, byte)
  return ok2 and idx or col
end

local function to_utf16(text, col, encoding)
  if encoding == "utf-16" then
    return col
  end
  local byte = col
  if encoding ~= "utf-8" then
    local ok, b = pcall(vim.str_byteindex, text, encoding, col)
    if not ok then
      return col
    end
    byte = b
  end
  local ok, idx = pcall(vim.str_utfindex, text, "utf-16", byte)
  return ok and idx or col
end

-- Re-encoding a result needs the text of the file it names, but not its LSP client.
local function text_line(uri, line)
  local b = vim.uri_to_bufnr(uri)
  if not vim.api.nvim_buf_is_loaded(b) then
    vim.fn.bufload(b)
  end
  return line_of(b, line)
end

local function range_to_utf16(uri, range, encoding)
  local function pos(p)
    return { line = p.line, col = to_utf16(text_line(uri, p.line), p.character, encoding) }
  end
  return { start = pos(range.start), ["end"] = pos(range["end"]) }
end

local function range_key(uri, range)
  return table.concat({
    uri,
    range.start.line,
    range.start.character,
    range["end"].line,
    range["end"].character,
  }, ":")
end

local function request(lsp_method, params, extra)
  local bufnr = M.bufnr_for(params.file)
  local uri = vim.uri_from_bufnr(bufnr)
  if #vim.lsp.get_clients({ bufnr = bufnr, method = lsp_method }) == 0 then
    return nil, uri
  end

  local text = line_of(bufnr, params.line)
  local results, err = vim.lsp.buf_request_sync(bufnr, lsp_method, function(client)
    return vim.tbl_extend("error", {
      textDocument = { uri = uri },
      position = { line = params.line, character = from_utf16(text, params.col, client.offset_encoding) },
    }, extra or {})
  end, TIMEOUT)
  if not results then
    error(err or "timeout")
  end
  return results, uri
end

local function encoding_of(client_id)
  local client = vim.lsp.get_client_by_id(client_id)
  return client and client.offset_encoding or "utf-16"
end

local function check(r)
  if r.err then
    error(r.err.message or vim.inspect(r.err))
  end
  return r.result
end

local function locations(lsp_method, params, extra)
  local results = request(lsp_method, params, extra)
  local out, seen = {}, {}
  for client_id, r in pairs(results or {}) do
    local result = check(r)
    local encoding = encoding_of(client_id)
    local items = (result and result.uri) and { result } or result or {}
    for _, item in ipairs(items) do
      local uri = item.targetUri or item.uri
      local range = item.targetUri and (item.targetSelectionRange or item.targetRange) or item.range
      if uri and range and not seen[range_key(uri, range)] then
        seen[range_key(uri, range)] = true
        out[#out + 1] = { file = vim.uri_to_fname(uri), range = range_to_utf16(uri, range, encoding) }
      end
    end
  end
  return out
end

local function highlights(params)
  local results, uri = request("textDocument/documentHighlight", params)
  local out, seen = {}, {}
  for client_id, r in pairs(results or {}) do
    local encoding = encoding_of(client_id)
    for _, item in ipairs(check(r) or {}) do
      if not seen[range_key(uri, item.range)] then
        seen[range_key(uri, item.range)] = true
        out[#out + 1] = {
          range = range_to_utf16(uri, item.range, encoding),
          kind = HIGHLIGHT_KIND[item.kind] or "text",
        }
      end
    end
  end
  return out
end

local function loaded_bufnr(file)
  for _, b in ipairs(vim.api.nvim_list_bufs()) do
    if vim.api.nvim_buf_is_loaded(b) and vim.api.nvim_buf_get_name(b) == file then
      return b
    end
  end
end

function M.text(file)
  local b = loaded_bufnr(file)
  if b then
    return table.concat(vim.api.nvim_buf_get_lines(b, 0, -1, false), "\n") .. "\n"
  end
  return table.concat(vim.fn.readfile(file), "\n") .. "\n"
end

local ANSWER = {
  ["host/text"] = function(params)
    return { text = M.text(params.file) }
  end,
  ["host/definition"] = function(params)
    return locations("textDocument/definition", params)
  end,
  ["host/references"] = function(params)
    return locations("textDocument/references", params, {
      context = { includeDeclaration = params.includeDeclaration and true or false },
    })
  end,
  ["host/documentHighlight"] = highlights,
}

function M.handle(method, params)
  local answer = ANSWER[method]
  if not answer then
    return nil, { code = METHOD_NOT_FOUND, message = "unknown method " .. tostring(method) }
  end
  local ok, result = pcall(answer, params)
  if not ok then
    return nil, { code = INTERNAL_ERROR, message = tostring(result) }
  end
  return result
end

return M
