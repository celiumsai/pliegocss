local function fail(message)
  vim.api.nvim_err_writeln("PliegoCSS Neovim host gate: " .. message)
  vim.cmd("cquit 1")
end

local function required(name)
  local value = vim.env[name]
  if not value or value == "" then
    fail("missing " .. name)
  end
  return value
end

local editor = required("PLIEGOCSS_NEOVIM_EDITOR")
local workspace = required("PLIEGOCSS_NEOVIM_WORKSPACE")
local output = required("PLIEGOCSS_NEOVIM_OUTPUT")
vim.opt.runtimepath:prepend(editor)

require("pliegocss").setup({
  server = required("PLIEGOCSS_NEOVIM_LSP"),
  root_dir = workspace,
  theme = { mode = "seed" },
  project_index = workspace .. "/out/pliego.index.json",
})

local source_path = workspace .. "/src/view.rs"
vim.cmd.edit(vim.fn.fnameescape(source_path))
local buffer = vim.api.nvim_get_current_buf()
vim.bo[buffer].filetype = "rust"

if not vim.wait(15000, function()
  return #vim.lsp.get_clients({ bufnr = buffer, name = "pliegocss" }) == 1
end, 25) then
  fail("language server did not attach")
end

local source = table.concat(vim.api.nvim_buf_get_lines(buffer, 0, -1, false), "\n")
local gap = assert(source:find("gap%-4", 1)) - 1
local responses = vim.lsp.buf_request_sync(buffer, "textDocument/definition", {
  textDocument = { uri = vim.uri_from_bufnr(buffer) },
  position = { line = 0, character = gap + 2 },
}, 15000)
local definition
for _, response in pairs(responses or {}) do
  if response.result and response.result[1] then
    definition = response.result[1]
  end
end
if not definition then
  fail("Project Index definition was not returned")
end
local target_uri = definition.targetUri or definition.uri
local target_range = definition.targetSelectionRange or definition.range
if vim.fs.normalize(vim.uri_to_fname(target_uri)) ~= vim.fs.normalize(workspace .. "/out/app.css") then
  fail("definition targeted the wrong stylesheet")
end
if target_range.start.line ~= 0 or target_range.start.character ~= 4 or target_range["end"].character ~= 16 then
  fail("definition targeted the wrong physical range")
end

local function replace(text)
  vim.api.nvim_buf_set_lines(buffer, 0, -1, false, { text })
end

local function diagnostic(code)
  local found
  if not vim.wait(15000, function()
    for _, value in ipairs(vim.diagnostic.get(buffer)) do
      if value.code == code then
        found = value
        return true
      end
    end
    return false
  end, 25) then
    fail("timed out waiting for " .. code)
  end
  return found
end

local invalid = 'fn view(){let _=pc!("flex unknown-thing");}'
replace(invalid)
local pcs = diagnostic("PCS001")
if pcs.message ~= "unknown utility `unknown-thing`" or pcs.col ~= invalid:find("unknown%-thing") - 1 then
  fail("PCS001 message or range drifted")
end

local pcx = 'fn view(){let _=pcx!("flex",if a{"opacity-50"}else{"block"},if b{"opacity-50"}else{"grid"});}'
replace(pcx)
local conflict = diagnostic("PCX003")
local first = assert(pcx:find('"opacity-50"', 1, true))
local second = assert(pcx:find('"opacity-50"', first + 1, true))
if not conflict.message:find("independent clauses 1 and 2", 1, true) or conflict.col ~= second - 1 then
  fail("PCX003 message or range drifted")
end

local version = vim.version()
local result = vim.json.encode({
  schemaVersion = 1,
  neovim = string.format("%d.%d.%d", version.major, version.minor, version.patch),
  client = "built-in-lsp",
  definition = "project-index-to-physical-css",
  diagnostics = { pcs = pcs.code, pcx = conflict.code },
  serverDownload = false,
})
vim.fn.writefile({ result }, output)
vim.cmd("qa!")
