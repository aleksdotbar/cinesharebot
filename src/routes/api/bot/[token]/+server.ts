import { error } from '@sveltejs/kit';
import { bot } from '$lib/bot';
import { handleUpdate } from '$lib/webhook';
import type { RequestHandler } from './$types';

export const POST: RequestHandler = async (req) => {
	if (req.params.token !== bot.token) throw error(404, 'Not Found');

	return handleUpdate(req);
};
