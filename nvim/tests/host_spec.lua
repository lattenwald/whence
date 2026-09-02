local fx = vim.fn.getcwd() .. "/engine/tests/fixtures/erlang/local_chain/a.erl"

describe("host", function()
  local host = require("whence.host")

  it("reads host/text from disk when no buffer is loaded", function()
    local result, err = host.handle("host/text", { file = fx })
    assert.is_nil(err)
    assert.equals("-module(a).", vim.split(result.text, "\n")[1])
    assert.equals("\n", result.text:sub(-1))
  end)

  it("prefers unsaved buffer content", function()
    local b = vim.fn.bufadd(fx)
    vim.fn.bufload(b)
    vim.api.nvim_buf_set_lines(b, 0, 1, false, { "-module(edited)." })
    local result = host.handle("host/text", { file = fx })
    assert.equals("-module(edited).", vim.split(result.text, "\n")[1])
    vim.api.nvim_buf_delete(b, { force = true })
  end)

  it("fails the request when no LSP client is attached", function()
    local scratch = vim.fn.tempname() .. ".txt"
    vim.fn.writefile({ "nothing here" }, scratch)
    local result, err = host.handle("host/definition", { file = scratch, line = 0, col = 0 })
    assert.is_nil(result)
    assert.equals("no language server attached to " .. scratch, err.message)
  end)

  it("waits for a client once per buffer until reset", function()
    local scratch = vim.fn.tempname() .. ".txt"
    vim.fn.writefile({ "nothing here" }, scratch)
    local function timed()
      local t0 = vim.uv.hrtime()
      host.bufnr_for(scratch)
      return (vim.uv.hrtime() - t0) / 1e6
    end
    host.reset()
    assert.is_true(timed() >= 1500)
    assert.is_true(timed() < 500)
    host.reset()
    assert.is_true(timed() >= 1500)
  end)

  it("rejects an unknown method", function()
    local result, err = host.handle("host/nope", {})
    assert.is_nil(result)
    assert.equals(-32601, err.code)
  end)

  it("leaves an already-open buffer listed and unlists one it opened", function()
    local open = vim.fn.tempname() .. ".txt"
    vim.fn.writefile({ "listed" }, open)
    vim.cmd.edit(vim.fn.fnameescape(open))
    assert.is_true(vim.bo[vim.fn.bufnr(open)].buflisted)
    assert.equals(vim.fn.bufnr(open), host.bufnr_for(open))
    assert.is_true(vim.bo[vim.fn.bufnr(open)].buflisted)

    local hidden = vim.fn.tempname() .. ".txt"
    vim.fn.writefile({ "hidden" }, hidden)
    assert.is_false(vim.bo[host.bufnr_for(hidden)].buflisted)
  end)
end)

describe("host locations", function()
  local host = require("whence.host")
  local uri = "file:///tmp/whence-test/x.erl"
  local file = "/tmp/whence-test/x.erl"
  local function line_at()
    return "aé𝄞b"
  end
  local function range(c1, c2)
    return { start = { line = 2, character = c1 }, ["end"] = { line = 2, character = c2 } }
  end

  it("flattens a bare Location", function()
    local out = host._locations_from({ { result = { uri = uri, range = range(7, 8) }, encoding = "utf-8" } }, line_at)
    assert.equals(1, #out)
    assert.equals(file, out[1].file)
    assert.same({ line = 2, col = 4 }, out[1].range.start)
    assert.same({ line = 2, col = 5 }, out[1].range["end"])
  end)

  it("flattens a Location list", function()
    local out = host._locations_from({
      { result = { { uri = uri, range = range(0, 1) }, { uri = uri, range = range(3, 7) } }, encoding = "utf-8" },
    }, line_at)
    assert.equals(2, #out)
    assert.equals(0, out[1].range.start.col)
    assert.equals(2, out[2].range.start.col)
  end)

  it("flattens LocationLinks by target selection range", function()
    local out = host._locations_from({
      {
        result = { { targetUri = uri, targetRange = range(0, 7), targetSelectionRange = range(3, 7) } },
        encoding = "utf-8",
      },
    }, line_at)
    assert.equals(1, #out)
    assert.same({ line = 2, col = 2 }, out[1].range.start)
  end)

  it("de-duplicates the same place reported by two clients in different encodings", function()
    local out = host._locations_from({
      { result = { { uri = uri, range = range(3, 7) } }, encoding = "utf-8" },
      { result = { { uri = uri, range = range(2, 3) } }, encoding = "utf-32" },
    }, line_at)
    assert.equals(1, #out)
    assert.same({ line = 2, col = 2 }, out[1].range.start)
  end)

  it("loads no buffer for a utf-16 client", function()
    local untouched = vim.fn.tempname() .. ".txt"
    vim.fn.writefile({ "aé𝄞b" }, untouched)
    local out = host._locations_from({
      { result = { { uri = vim.uri_from_fname(untouched), range = range(0, 1) } }, encoding = "utf-16" },
    })
    assert.equals(untouched, out[1].file)
    assert.equals(0, vim.fn.bufexists(untouched))
  end)
end)
