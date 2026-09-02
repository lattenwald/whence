local M = {}

local util = require("whence.util")
local CODES = vim.lsp.protocol.ErrorCodes

local function absolutise(dir, locations)
  for _, loc in ipairs(locations) do
    if vim.startswith(loc.file, "$HOME/") then
      loc.file = vim.env.HOME .. loc.file:sub(#"$HOME" + 1)
    elseif not vim.startswith(loc.file, "/") then
      loc.file = dir .. "/" .. loc.file
    end
  end
  return locations
end

local function load(dir)
  local recorded = vim.json.decode(table.concat(vim.fn.readfile(dir .. "/host.json"), "\n"))
  for _, section in ipairs({ "definition", "references" }) do
    for _, locations in pairs(recorded[section] or {}) do
      absolutise(dir, locations)
    end
  end
  return recorded
end

function M.handle(dir)
  local recorded = load(dir)
  return function(method, params)
    if method == "host/text" then
      return { text = table.concat(vim.fn.readfile(params.file), "\n") .. "\n" }
    end
    local section = method:match("^host/(.+)$")
    if not section or not recorded[section] then
      return nil, { code = CODES.MethodNotFound, message = "unknown method " .. tostring(method) }
    end
    local answer = recorded[section][util.fixture_key(dir, method, params)]
    if not answer then
      return nil, { code = CODES.InternalError, message = "unrecorded" }
    end
    return answer
  end
end

return M
