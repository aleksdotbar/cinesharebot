import * as R from "remeda"
import { TMDB, MovieWithMediaType, TVWithMediaType } from "tmdb-ts"

export type QueryResult = {
  type: string
  id: string
  title: string
  poster: { url: string; width: number; height: number }
  thumb: { url: string; width: number; height: number }
  year: string | null
}

const IMAGINARY_URL = Bun.env.IMAGINARY_URL

if (!IMAGINARY_URL) {
  throw new Error("IMAGINARY_URL is not set")
}

// const imageUrl = (path: string, w: 92 | 154 | 185 | 342 | 500 | 780) =>
//   `${IMAGINARY_URL}/resize?width=${w}&height=${w * 1.5}&url=https://image.tmdb.org/t/p/w${w}${path}`

const imageUrl = (path: string, w: 92 | 154 | 185 | 342 | 500 | 780) =>
  `https://cineshare.vercel.app/api/image?url=https://image.tmdb.org/t/p/w${w}${path}`

const isMovie = (m: { media_type: unknown }): m is MovieWithMediaType => m.media_type === "movie"

const isTV = (m: { media_type: unknown }): m is TVWithMediaType => m.media_type === "tv"

const isMovieOrTV = (m: { media_type: unknown }): m is MovieWithMediaType | TVWithMediaType =>
  isMovie(m) || isTV(m)

const parseMovieOrTV = (m: MovieWithMediaType | TVWithMediaType): QueryResult => ({
  id: String(m.id),
  type: m.media_type,
  poster: { url: imageUrl(m.poster_path, 500), width: 500, height: 750 },
  thumb: { url: imageUrl(m.poster_path, 154), width: 154, height: 231 },
  title: isMovie(m) ? m.title : m.name,
  year: (isMovie(m) ? m.release_date : m.first_air_date)?.slice(0, 4) ?? null,
})

const api = new TMDB(Bun.env.TMDB_TOKEN ?? "")

export const getQueryResults = async (query: string): Promise<QueryResult[]> =>
  R.pipe(
    query ? await api.search.multi({ query }) : await api.trending.trending("all", "week"),
    R.prop("results"),
    R.filter(isMovieOrTV),
    R.filter((m) => !!m.poster_path),
    R.map(parseMovieOrTV)
  )
