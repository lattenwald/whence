local M = {}

local TIMEOUT = 5000
local METHOD_NOT_FOUND = -32601
local INTERNAL_ERROR = -32603

local HIGHLIGHT_KIND = { "text", "read", "write" }

function M.bufnr_for(file)
  local fresh = vim.fn.bufexists(file) == 0
  local b = vim.fn.bufadd(file)
  vim.fn.bufload(b)
  if fresh then
    vim.bo[b].buflisted = false
  end
  vim.wait(2000, function()
    return #vim.lsp.get_clients({ bufnr = b }) > 0
  end)
  return b
end

local function line_of(bufnr, line)
  return vim.api.nvim_buf_get_lines(bufnr, line, line + 1, false)[1] or ""
end

function M._from_utf16(text, col, encoding)
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

function M._to_utf16(text, col, encoding)
  if encoding == "utf-16" then
    return col
  end
  -- A utf-8 column already is the byte index; other encodings need the lookup.
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

local function buffer_line(file, line)
  local b = vim.uri_to_bufnr(vim.uri_from_fname(file))
  if not vim.api.nvim_buf_is_loaded(b) then
    vim.fn.bufload(b)
  end
  return line_of(b, line)
end

local function range_to_utf16(range, encoding, line_at)
  local function pos(p)
    local col = encoding == "utf-16" and p.character or M._to_utf16(line_at(p.line), p.character, encoding)
    return { line = p.line, col = col }
  end
  return { start = pos(range.start), ["end"] = pos(range["end"]) }
end

local function range_key(file, range)
  return table.concat({
    file,
    range.start.line,
    range.start.col,
    range["end"].line,
    range["end"].col,
  }, ":")
end

local function items_of(result)
  if not result then
    return {}
  end
  if result.uri or result.targetUri then
    return { result }
  end
  return result
end

function M._locations_from(per_client, line_at)
  line_at = line_at or buffer_line
  local out, seen = {}, {}
  for _, entry in ipairs(per_client) do
    for _, item in ipairs(items_of(entry.result)) do
      local uri = item.targetUri or item.uri
      local range = item.targetUri and (item.targetSelectionRange or item.targetRange) or item.range
      if uri and range then
        local file = vim.uri_to_fname(uri)
        local converted = range_to_utf16(range, entry.encoding, function(line)
          return line_at(file, line)
        end)
        local key = range_key(file, converted)
        if not seen[key] then
          seen[key] = true
          out[#out + 1] = { file = file, range = converted }
        end
      end
    end
  end
  return out
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
      position = { line = params.line, character = M._from_utf16(text, params.col, client.offset_encoding) },
    }, extra or {})
  end, TIMEOUT)
  if not results then
    error(err or "timeout")
  end
  return results, uri
end

-- A failing client is dropped while another answers; only an all-client failure aborts the trace (spec §4).
local function answers(results)
  local out, failed = {}, {}
  for client_id, r in pairs(results or {}) do
    local client = vim.lsp.get_client_by_id(client_id)
    if r.err then
      local name = client and client.name or ("client " .. client_id)
      failed[#failed + 1] = name .. ": " .. (r.err.message or vim.inspect(r.err))
    else
      out[#out + 1] = { result = r.result, encoding = client and client.offset_encoding or "utf-16" }
    end
  end
  if #failed > 0 then
    if #out == 0 then
      error(table.concat(failed, "; "))
    end
    vim.notify("whence: " .. table.concat(failed, "; "), vim.log.levels.WARN)
  end
  return out
end

local function locations(lsp_method, params, extra)
  return M._locations_from(answers(request(lsp_method, params, extra)))
end

local function highlights(params)
  local results, uri = request("textDocument/documentHighlight", params)
  local file = vim.uri_to_fname(uri)
  local out, seen = {}, {}
  for _, entry in ipairs(answers(results)) do
    for _, item in ipairs(entry.result or {}) do
      local range = range_to_utf16(item.range, entry.encoding, function(line)
        return buffer_line(file, line)
      end)
      local key = range_key(file, range)
      if not seen[key] then
        seen[key] = true
        out[#out + 1] = { range = range, kind = HIGHLIGHT_KIND[item.kind] or "text" }
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
