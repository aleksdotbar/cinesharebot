import { Bot, InlineQueryResultBuilder } from "grammy";
import { getSearchResults, getTreandingResults, MediaResult } from "./api";
import { description, imageUrl } from "./utils";

export const bot = new Bot(Bun.env.BOT_TOKEN ?? "", { client: { canUseWebhookReply: () => true } });

bot.on("message", (ctx) => {
  if (!ctx.message.text) return;

  ctx.reply(description(ctx.me.username), { parse_mode: "MarkdownV2" });
});

const toPhotoResult = (m: MediaResult) => {
  const emoji = m.type === "movie" ? "📽" : "📺";
  const title = `${m.title}${m.year ? ` (${m.year})` : ""}`;

  return InlineQueryResultBuilder.photo(m.id, imageUrl(m.poster.url), {
    thumbnail_url: imageUrl(m.thumb.url),
    photo_width: m.poster.width,
    photo_height: m.poster.height,
    caption: `${emoji} <b>${title}</b>`,
    parse_mode: "HTML",
  });
};

bot.on("inline_query", async (ctx) => {
  try {
    const query = ctx.inlineQuery.query.trim();

    const results = query ? await getSearchResults(query) : await getTreandingResults();

    await ctx.answerInlineQuery(results.map(toPhotoResult), { cache_time: 0 });
  } catch (error) {
    console.error(error);
  }
});
