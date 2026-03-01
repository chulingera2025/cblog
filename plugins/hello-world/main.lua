-- 注册一个构建开始的 action hook
plugin.action("after_load", 10, function(ctx)
    cblog.log.info("Hello from hello-world plugin!")
    cblog.log.info("共加载 " .. #ctx.posts .. " 篇文章")
end)

-- 注册一个 after_render action hook 示例
plugin.action("after_render", 10, function(ctx)
    cblog.log.debug("hello-world: 输出目录 = " .. (ctx.output_dir or "unknown"))
end)
