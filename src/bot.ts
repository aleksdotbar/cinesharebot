import { Bot, InlineQueryResultBuilder } from "grammy";
import { getSearchResults, getTreandingResults, MediaResult } from "./api";
import { description, imageUrl } from "./utils";
import Redis from "ioredis";

const BOT_TOKEN = Bun.env.BOT_TOKEN;

if (!BOT_TOKEN) throw new Error("BOT_TOKEN is not set");

const ANALYTICS_CHAT_ID = Bun.env.ANALYTICS_CHAT_ID;

if (!ANALYTICS_CHAT_ID) throw new Error("ANALYTICS_CHAT_ID is not set");

export const bot = new Bot(BOT_TOKEN, {
  client: {
    canUseWebhookReply: (method) => method === "answerInlineQuery",
  },
});

const REDIS_URL = Bun.env.REDIS_URL;

if (!REDIS_URL) throw new Error("REDIS_URL is not set");

const redis = new Redis(REDIS_URL);

type UserConfig = {
  caption: boolean;
};

const defaultConfig: UserConfig = {
  caption: true,
};

const parseUserConfig = (config: string | null) =>
  config ? (JSON.parse(config) as UserConfig) : defaultConfig;

const toPhotoResult = (cfg: UserConfig) => (res: MediaResult) => {
  const emoji = res.type === "movie" ? "📽" : "📺";
  const title = `${res.title}${res.year ? ` (${res.year})` : ""}`;

  return InlineQueryResultBuilder.photo(res.id, imageUrl(res.poster.url), {
    thumbnail_url: imageUrl(res.thumb.url),
    photo_width: res.poster.width,
    photo_height: res.poster.height,
    caption: cfg.caption ? `${emoji} <b>${title}</b>` : undefined,
    parse_mode: "HTML",
  });
};

bot.on("inline_query", async (ctx) => {
  try {
    const query = ctx.inlineQuery.query.trim();

    const results = query ? await getSearchResults(query) : await getTreandingResults();

    const settings = await redis.get(`settings:${ctx.from.id}`).then(parseUserConfig);

    await ctx.answerInlineQuery(results.map(toPhotoResult(settings)), { cache_time: 0 });
  } catch (error) {
    console.error(error);
  }
});

bot.on("chosen_inline_result", async (ctx) => {
  try {
    const { query, result_id } = ctx.chosenInlineResult;

    const message = query
      ? `Someone searched for \`${query}\` and chose a result with id \`${result_id}\``
      : `Someone chose a trending result with id \`${result_id}\``;

    await bot.api.sendMessage(ANALYTICS_CHAT_ID, `Hey, yo\\!\n\n${message}`, {
      parse_mode: "MarkdownV2",
      disable_notification: true,
    });
  } catch (error) {
    console.error(error);
  }
});

bot.on("message", (ctx) => {
  if (!ctx.message.text) return;

  ctx.reply(description(ctx.me.username), { parse_mode: "MarkdownV2" });
});
