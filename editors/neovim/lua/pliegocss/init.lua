local M = {}

local function absolute_file(path, label)
  assert(type(path) == "string" and path ~= "", label .. " path is required")
  local absolute = vim.fs.abspath(path)
  assert(vim.fn.filereadable(absolute) == 1, label .. " path is not a readable file: " .. absolute)
  return absolute
end

local function absolute_executable(path, label)
  local absolute = absolute_file(path, label)
  assert(vim.fn.executable(absolute) == 1, label .. " path is not executable: " .. absolute)
  return absolute
end

local function command(options)
  local server = absolute_executable(options.server, "pliego-css-lsp")
  local compiler = absolute_executable(options.compiler, "pliego-cssc")
  local args = { server, "--pliego-cssc", compiler }
  local theme = options.theme or { mode = "discover" }
  if theme.mode == "seed" then
    table.insert(args, "--seed")
  elseif theme.mode == "config" then
    table.insert(args, "--config")
    table.insert(args, absolute_file(theme.path, "theme config"))
  else
    assert(theme.mode == "discover", "theme mode must be discover, seed, or config")
  end
  if options.project_index then
    table.insert(args, "--project-index")
    table.insert(args, absolute_file(options.project_index, "Project Index"))
  end
  return args
end

function M.start(options, buffer)
  options = options or {}
  buffer = buffer or vim.api.nvim_get_current_buf()
  local root = vim.fs.abspath(options.root_dir or vim.fn.getcwd())
  assert(vim.fn.isdirectory(root) == 1, "root_dir is not a directory: " .. root)
  return vim.lsp.start({
    name = "pliegocss",
    cmd = command(options),
    root_dir = root,
    filetypes = { "rust" },
  }, { bufnr = buffer })
end

function M.setup(options)
  assert(type(options) == "table", "pliegocss.setup requires explicit options")
  local group = vim.api.nvim_create_augroup("PliegoCssLsp", { clear = true })
  vim.api.nvim_create_autocmd("FileType", {
    group = group,
    pattern = "rust",
    callback = function(event)
      if vim.bo[event.buf].buftype ~= "" or vim.api.nvim_buf_get_name(event.buf) == "" then
        return
      end
      M.start(options, event.buf)
    end,
  })
end

return M
