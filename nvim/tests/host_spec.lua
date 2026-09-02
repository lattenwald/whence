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

  it("answers an empty list when no LSP client is attached", function()
    local scratch = vim.fn.tempname() .. ".txt"
    vim.fn.writefile({ "nothing here" }, scratch)
    local result, err = host.handle("host/definition", { file = scratch, line = 0, col = 0 })
    assert.is_nil(err)
    assert.same({}, result)
  end)

  it("rejects an unknown method", function()
    local result, err = host.handle("host/nope", {})
    assert.is_nil(result)
    assert.equals(-32601, err.code)
  end)
end)
