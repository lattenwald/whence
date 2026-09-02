local fx = vim.fn.getcwd() .. "/engine/tests/fixtures/erlang/local_chain"

describe("whence.setup", function()
  it("traces through a real `whence serve` with the replay host", function()
    local whence = require("whence")
    whence.setup({ _replay = fx, root = fx })

    local panel = require("whence.panel")
    panel.last = nil
    whence.trace_at(fx .. "/a.erl", 6, 4)
    vim.wait(10000, function()
      return panel.last ~= nil
    end)

    assert.is_not_nil(panel.last)
    assert.equals("Z", panel.last.tree.root.label)
    whence.stop()
  end)
end)
