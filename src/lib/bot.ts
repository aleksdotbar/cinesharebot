import * as R from 'remeda';
import { Bot, InlineQueryResultBuilder } from 'grammy';
import { getQueryResults, type QueryResult } from './api';
import { env } from '$env/dynamic/private';

export const bot = new Bot(env.BOT_TOKEN ?? '', {
	client: { canUseWebhookReply: () => true }
});

const description = (username: string) => `
This bot can help you find and share movies and tv shows\\.

It works automatically, no need to add it anywhere\\.

Simply open any of your chats, type \`@${username}\` and a movie or a tv show title after a space\\.
Then tap on one of the results\\. 

If you just type \`@${username}\` and a space, you'll see trending results\\.
    
If you already have some messages from this bot in your chat, you can just tap on bot's username and it will be pasted into the input field\\.
    
You can try it right here\\.`;

bot.on('message', (ctx) => {
	if (!ctx.message.text) return;

	ctx.reply(description(ctx.me.username), { parse_mode: 'MarkdownV2' });
});

const toPhotoResult = (m: QueryResult) => {
	const emoji = m.type === 'movie' ? '📽' : '📺';
	const title = `${m.title}${m.year ? ` (${m.year})` : ''}`;

	return InlineQueryResultBuilder.photo(m.id, m.poster.url, {
		thumbnail_url: m.thumb.url,
		photo_width: m.poster.width,
		photo_height: m.poster.height,
		caption: `${emoji} <b>${title}</b>`,
		parse_mode: 'HTML'
	});
};

bot.on('inline_query', async (ctx) => {
	try {
		// const results = R.pipe(
		// 	await getQueryResults(ctx.inlineQuery.query.trim()),
		// 	R.map(toPhotoResult)
		// );

		const results = [
			toPhotoResult({
				id: '1',
				type: 'movie',
				title: 'Spider-Man: Across the Spider-Verse',
				year: '2022',
				poster: {
					url: `https://${env.VERCEL_URL}/api/image?url=https://image.tmdb.org/t/p/w500/8Vt6mWEReuy4Of61Lnj5Xj704m8.jpg`,
					width: 500,
					height: 750
				},
				thumb: {
					url: `https://${env.VERCEL_URL}/api/image?url=https://image.tmdb.org/t/p/w154/8Vt6mWEReuy4Of61Lnj5Xj704m8.jpg`,
					width: 154,
					height: 231
				}
			})
		];

		ctx.answerInlineQuery(results, { cache_time: 0 });
	} catch (error) {
		console.error(error);
	}
});
