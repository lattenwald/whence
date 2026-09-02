local fx = vim.fn.getcwd() .. "/engine/tests/fixtures/erlang/local_chain"

describe("host_replay", function()
  local handle = require("whence.host_replay").handle(fx)

  it("answers a recorded definition with absolute paths", function()
    local result, err = handle("host/definition", { file = fx .. "/a.erl", line = 6, col = 4 })
    assert.is_nil(err)
    assert.equals(1, #result)
    assert.equals(fx .. "/a.erl", result[1].file)
    assert.same({ line = 5, col = 4 }, result[1].range.start)
  end)

  it("errors on an unrecorded position", function()
    local result, err = handle("host/definition", { file = fx .. "/a.erl", line = 0, col = 0 })
    assert.is_nil(result)
    assert.is_not_nil(err)
    assert.equals("unrecorded", err.message)
  end)

  it("distinguishes includeDeclaration in reference keys", function()
    local result, err = handle("host/references", { file = fx .. "/a.erl", line = 3, col = 0, includeDeclaration = false })
    assert.is_nil(err)
    assert.same({}, result)
    local _, derr = handle("host/references", { file = fx .. "/a.erl", line = 3, col = 0, includeDeclaration = true })
    assert.is_not_nil(derr)
  end)
end)
