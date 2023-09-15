import { TMDB, MovieWithMediaType, TVWithMediaType } from "tmdb-ts"

export type QueryResult = {
  type: string
  id: string
  poster: string
  thumb: string
  title: string
  year: string | null
}

type Width = (typeof Width)[keyof typeof Width]

const Width = {
  W154: 154,
  W500: 500,
} as const

const imageUrl = (path: string, w: number) => `https://image.tmdb.org/t/p/w${w}${path}`

const isMovie = (m: { media_type: unknown }): m is MovieWithMediaType => m.media_type === "movie"

const isTV = (m: { media_type: unknown }): m is TVWithMediaType => m.media_type === "tv"

const isMovieOrTV = (m: { media_type: unknown }): m is MovieWithMediaType | TVWithMediaType =>
  isMovie(m) || isTV(m)

const parseMovieOrTV = (m: MovieWithMediaType | TVWithMediaType): QueryResult => ({
  type: m.media_type,
  id: String(m.id),
  poster: imageUrl(m.poster_path, Width.W500),
  thumb: imageUrl(m.poster_path, Width.W154),
  title: isMovie(m) ? m.title : m.name,
  year: (isMovie(m) ? m.release_date : m.first_air_date)?.slice(0, 4) ?? null,
})

const api = new TMDB(Bun.env.TMDB_TOKEN ?? "")

export const getQueryResults = async (q: string): Promise<QueryResult[]> => {
  const data = q ? await api.search.multi({ query: q }) : await api.trending.trending("all", "week")

  return data.results.filter(isMovieOrTV).map(parseMovieOrTV)
}
