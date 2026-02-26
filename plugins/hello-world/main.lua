-- 注册一个构建开始的 action hook
plugin.action("after_load", 10, function(ctx)
    cblog.log.info("Hello from hello-world plugin!")
end)

-- 注册一个简单的 filter hook 示例
plugin.filter("after_render", 10, function(page)
    -- 不修改内容，只记录日志
    cblog.log.debug("hello-world: page rendered")
    return page
end)
