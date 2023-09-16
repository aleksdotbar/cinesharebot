import * as R from 'remeda';
import { TMDB } from 'tmdb-ts';
import type { MovieWithMediaType, TVWithMediaType } from 'tmdb-ts';

export type QueryResult = {
	type: string;
	id: string;
	title: string;
	poster: { url: string; width: number; height: number };
	thumb: { url: string; width: number; height: number };
	year: string | null;
};

type Width = 92 | 154 | 185 | 342 | 500 | 780;

// const IMAGINARY_URL = Bun.env.IMAGINARY_URL;

// if (!IMAGINARY_URL) {
// 	throw new Error('IMAGINARY_URL is not set');
// }

// const imageUrl = (path: string, w: 92 | 154 | 185 | 342 | 500 | 780) =>
//   `${IMAGINARY_URL}/resize?width=${w}&height=${w * 1.5}&url=https://image.tmdb.org/t/p/w${w}${path}`

const imageUrl = (path: string, w: Width) => `https://image.tmdb.org/t/p/w${w}${path}`;

const isMovie = (m: { media_type: unknown }): m is MovieWithMediaType => m.media_type === 'movie';

const isTV = (m: { media_type: unknown }): m is TVWithMediaType => m.media_type === 'tv';

const isMovieOrTV = (m: { media_type: unknown }): m is MovieWithMediaType | TVWithMediaType =>
	isMovie(m) || isTV(m);

const posterWidth: Width = 500;
const posterHeight = posterWidth * 1.5;
const thumbWidth: Width = 154;
const thumbHeight = thumbWidth * 1.5;

const parseMovieOrTV = (m: MovieWithMediaType | TVWithMediaType): QueryResult => ({
	id: String(m.id),
	type: m.media_type,
	poster: { url: imageUrl(m.poster_path, posterWidth), width: posterWidth, height: posterHeight },
	thumb: { url: imageUrl(m.poster_path, thumbWidth), width: thumbWidth, height: thumbHeight },
	title: isMovie(m) ? m.title : m.name,
	year: (isMovie(m) ? m.release_date : m.first_air_date)?.slice(0, 4) ?? null
});

const api = new TMDB(import.meta.env.TMDB_TOKEN ?? '');

export const getQueryResults = async (query: string): Promise<QueryResult[]> =>
	R.pipe(
		query ? await api.search.multi({ query }) : await api.trending.trending('all', 'week'),
		R.prop('results'),
		R.filter(isMovieOrTV),
		R.filter((m) => !!m.poster_path),
		R.map(parseMovieOrTV)
	);
