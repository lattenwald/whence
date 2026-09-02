describe("install", function()
  local install = require("whence.install")

  it("maps LuaJIT os/arch to release targets", function()
    assert.equals("x86_64-unknown-linux-gnu", install.target("Linux", "x64"))
    assert.equals("aarch64-unknown-linux-gnu", install.target("Linux", "arm64"))
    assert.equals("x86_64-apple-darwin", install.target("OSX", "x64"))
    assert.equals("aarch64-apple-darwin", install.target("OSX", "arm64"))
    assert.equals("x86_64-pc-windows-msvc", install.target("Windows", "x64"))
  end)

  it("reports an unsupported platform instead of guessing", function()
    local target, err = install.target("Linux", "ppc64")
    assert.is_nil(target)
    assert.is_truthy(err:find("ppc64"))
  end)

  it("builds the release urls", function()
    local urls = install.urls("lattenwald/whence", "0.1.0", "x86_64-apple-darwin")
    assert.equals(
      "https://github.com/lattenwald/whence/releases/download/v0.1.0/whence-x86_64-apple-darwin.tar.gz",
      urls.tarball
    )
    assert.equals("https://github.com/lattenwald/whence/releases/download/v0.1.0/SHA256SUMS", urls.sums)
    assert.equals("whence-x86_64-apple-darwin.tar.gz", urls.archive)
  end)

  it("parses SHA256SUMS", function()
    local sums = install.parse_sums(table.concat({
      "aa11  whence-x86_64-unknown-linux-gnu.tar.gz",
      "bb22 *whence-x86_64-pc-windows-msvc.tar.gz",
      "",
      "# a comment",
    }, "\n"))
    assert.equals("aa11", sums["whence-x86_64-unknown-linux-gnu.tar.gz"])
    assert.equals("bb22", sums["whence-x86_64-pc-windows-msvc.tar.gz"])
    assert.is_nil(sums["# a comment"])
  end)

  it("ships a version matching the engine crate", function()
    local crate = table.concat(vim.fn.readfile(vim.fn.getcwd() .. "/engine/Cargo.toml"), "\n")
    assert.equals(crate:match('\nversion%s*=%s*"([^"]+)"'), require("whence.version"))
  end)
end)
