import { webhookCallback } from "grammy"
import { bot } from "./bot"

const handleUpdate = webhookCallback(bot, "std/http")

Bun.serve({
  async fetch(req) {
    const url = new URL(req.url)

    if (req.method !== "POST" || url.pathname.slice(1) !== bot.token) {
      return new Response()
    }

    try {
      return await handleUpdate(req)
    } catch (err) {
      console.error(err)
    }
  },
})
