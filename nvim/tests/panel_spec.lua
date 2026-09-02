local fx = vim.fn.getcwd() .. "/engine/tests/fixtures/erlang/local_chain"

-- Fixture swap files outlive this instance and trip the other specs' E325.
vim.o.swapfile = false

describe("panel.render", function()
  it("renders nodes, via, location and stops", function()
    local tree = {
      root = {
        id = "a",
        kind = "binding",
        label = "Z",
        loc = { file = "/p/a.erl", line = 5, col = 4 },
        via = "match",
        snippet = "Z.",
        stop = vim.NIL,
        truncated = 0,
        children = {
          {
            id = "b",
            kind = "param",
            label = "X",
            loc = { file = "/p/a.erl", line = 2, col = 2 },
            via = "match",
            snippet = "f(X) ->",
            stop = vim.NIL,
            truncated = 2,
            children = {
              {
                id = "c",
                kind = "stop",
                label = "X",
                loc = { file = "/p/a.erl", line = 2, col = 2 },
                via = vim.NIL,
                snippet = "f(X) ->",
                stop = { reason = "entry_point", detail = "no call sites of f/1" },
                truncated = 0,
                children = {},
              },
            },
          },
        },
      },
    }

    local lines, index = require("whence.panel").render(tree, "/p")
    assert.equals("● Z  ← match  a.erl:6:5", lines[1])
    assert.equals("  ● X  ← match  a.erl:3:3", lines[2])
    assert.equals("    ■ X  a.erl:3:3  [entry_point: no call sites of f/1]", lines[3])
    assert.equals("    … 2 more", lines[4])
    assert.equals("c", index[3].id)
    assert.is_nil(index[4])
  end)

  it("marks a branch node as a value, not a stop", function()
    local tree = {
      root = {
        id = "a",
        kind = "branch",
        label = "case K of",
        loc = { file = "/p/a.erl", line = 4, col = 8 },
        via = "match",
        snippet = "Z = case K of",
        stop = vim.NIL,
        truncated = 0,
        children = {},
      },
    }
    local lines = require("whence.panel").render(tree, "/p")
    assert.equals("● case K of  ← match  a.erl:5:9", lines[1])
  end)

  it("keeps absolute paths that are not under the root", function()
    local tree = {
      root = {
        id = "a",
        kind = "binding",
        label = "Z",
        loc = { file = "/elsewhere/b.erl", line = 0, col = 0 },
        via = vim.NIL,
        stop = vim.NIL,
        truncated = 0,
        children = {},
      },
    }
    local lines = require("whence.panel").render(tree, "/p")
    assert.equals("● Z  /elsewhere/b.erl:1:1", lines[1])
  end)
end)

describe("panel.show", function()
  it("opens a whence buffer and jumps on <CR>", function()
    vim.cmd.edit(fx .. "/a.erl")
    local source = vim.api.nvim_get_current_buf()

    require("whence").setup({ bin = vim.g.whence_bin, _replay = fx, root = fx })
    require("whence").trace_at(fx .. "/a.erl", 6, 4)
    vim.wait(10000, function()
      return vim.bo.filetype == "whence"
    end)
    assert.equals("whence", vim.bo.filetype)

    local panel_buf = vim.api.nvim_get_current_buf()
    assert.same({
      "● Z  ← match  a.erl:6:5",
      "  ● Y  ← match  a.erl:5:5",
      "    ● X  ← match  a.erl:4:3",
      "      ■ X  a.erl:4:3  [entry_point: no call sites of f/1]",
    }, vim.api.nvim_buf_get_lines(panel_buf, 0, -1, false))
    assert.is_false(vim.bo[panel_buf].modifiable)

    vim.api.nvim_win_set_cursor(0, { 2, 0 })
    vim.api.nvim_feedkeys(vim.api.nvim_replace_termcodes("<CR>", true, false, true), "x", false)
    assert.equals(source, vim.api.nvim_get_current_buf())
    assert.equals("erlang", vim.bo.filetype)
    assert.same({ 5, 4 }, vim.api.nvim_win_get_cursor(0))

    require("whence").stop()
  end)

  it("previews without leaving the panel and re-runs from a node", function()
    vim.cmd.edit(fx .. "/a.erl")
    require("whence").setup({ bin = vim.g.whence_bin, _replay = fx, root = fx })
    require("whence").trace_at(fx .. "/a.erl", 6, 4)
    vim.wait(10000, function()
      return vim.bo.filetype == "whence"
    end)

    local panel_buf = vim.api.nvim_get_current_buf()
    vim.api.nvim_win_set_cursor(0, { 3, 0 })
    require("whence.panel").preview_current()
    assert.equals(panel_buf, vim.api.nvim_get_current_buf())

    local whence = require("whence")
    local original, seen = whence.trace_at, nil
    whence.trace_at = function(...)
      seen = { ... }
    end
    require("whence.panel").rerun_current()
    whence.trace_at = original
    assert.same({ fx .. "/a.erl", 3, 2 }, seen)
    assert.equals(panel_buf, vim.api.nvim_get_current_buf())

    vim.api.nvim_feedkeys(vim.api.nvim_replace_termcodes("q", true, false, true), "x", false)
    assert.is_not.equals(panel_buf, vim.api.nvim_get_current_buf())

    require("whence").stop()
  end)
end)
