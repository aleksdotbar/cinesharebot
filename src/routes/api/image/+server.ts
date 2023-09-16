import { error } from '@sveltejs/kit';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ url }) => {
	const imageUrl = url.searchParams.get('url');

	if (!imageUrl) throw error(400, 'No image url provided');

	return new Response(await fetch(imageUrl).then((res) => res.arrayBuffer()));
};
