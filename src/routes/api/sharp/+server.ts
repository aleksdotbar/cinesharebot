import sharp from 'sharp';
import { error } from '@sveltejs/kit';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ url }) => {
	const params = url.searchParams;
	const imageUrl = params.get('url');
	const width = Number(params.get('width'));

	if (!imageUrl || !width) throw error(400, 'Missing parameters');

	const imageBuffer = await fetch(imageUrl).then((res) => res.arrayBuffer());

	const resisedBuffer = await sharp(imageBuffer).resize(width).toBuffer();

	return new Response(resisedBuffer);
};
