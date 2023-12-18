import { TMDB, MovieWithMediaType, TVWithMediaType, MultiSearchResult } from "tmdb-ts";

const api = new TMDB(Bun.env.TMDB_TOKEN ?? "");

export const POSTER_WIDTH = 780;

export const THUMB_WIDTH = 154;

export const getSearchResults = async (query: string): Promise<Array<MediaResult>> => {
  const { results } = await api.search.multi({ query });

  return parseResults(results);
};

export const getTreandingResults = async (): Promise<Array<MediaResult>> => {
  const { results } = await api.trending.trending("all", "week");

  return parseResults(results);
};

export const isMovie = (m: { media_type: unknown }): m is MovieWithMediaType =>
  m.media_type === "movie";

export const isTV = (m: { media_type: unknown }): m is TVWithMediaType => m.media_type === "tv";

export type MediaResult = {
  type: string;
  id: string;
  title: string;
  poster: { url: string; width: number; height: number };
  thumb: { url: string; width: number; height: number };
  year: string | null;
};

const imageUrl = (path: string, w: 92 | 154 | 185 | 342 | 500 | 780) =>
  `https://image.tmdb.org/t/p/w${w}${path}`;

const isMovieOrTV = (m: { media_type: unknown }): m is MovieWithMediaType | TVWithMediaType =>
  isMovie(m) || isTV(m);

const parseMovieOrTV = (m: MovieWithMediaType | TVWithMediaType): MediaResult => ({
  id: String(m.id),
  type: m.media_type,
  title: isMovie(m) ? m.title : m.name,
  year: (isMovie(m) ? m.release_date : m.first_air_date)?.slice(0, 4) ?? null,
  poster: {
    url: imageUrl(m.poster_path, POSTER_WIDTH),
    width: POSTER_WIDTH,
    height: POSTER_WIDTH * 1.5,
  },
  thumb: {
    url: imageUrl(m.poster_path, THUMB_WIDTH),
    width: POSTER_WIDTH,
    height: POSTER_WIDTH * 1.5,
  },
});

const parseResults = (results: Array<MultiSearchResult>): Array<MediaResult> =>
  results
    .filter(isMovieOrTV)
    .filter((m) => !!m.poster_path)
    .map(parseMovieOrTV);
