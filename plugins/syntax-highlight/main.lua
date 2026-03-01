plugin.action("after_render", 15, function(ctx)
    local output = ctx.output_dir or "public"

    local css = [[
<style>
/* syntect 代码高亮主题 */
.code-highlight { background: #282c34; color: #abb2bf; padding: 16px; border-radius: 6px; overflow-x: auto; }
.code-highlight code { background: none; padding: 0; }
.code-highlight .source { color: #abb2bf; }
.code-highlight .comment { color: #5c6370; font-style: italic; }
.code-highlight .string { color: #98c379; }
.code-highlight .constant { color: #d19a66; }
.code-highlight .keyword { color: #c678dd; }
.code-highlight .storage { color: #c678dd; }
.code-highlight .entity { color: #61afef; }
.code-highlight .variable { color: #e06c75; }
.code-highlight .support { color: #56b6c2; }
.code-highlight .punctuation { color: #abb2bf; }
.code-highlight .meta { color: #abb2bf; }
</style>
]]

    if cblog.files.exists(output) then
        local files = cblog.files.list(output)
        if files then
            for _, name in ipairs(files) do
                if string.match(name, "%.html$") then
                    local path = output .. "/" .. name
                    local content = cblog.files.read(path)
                    if content and string.find(content, "code%-highlight") then
                        content = string.gsub(content, "</head>", css .. "</head>")
                        cblog.files.write(path, content)
                    end
                end
            end
        end
    end

    cblog.log.debug("syntax-highlight: 高亮样式已注入")
end)
