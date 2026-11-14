import type { MediaState } from "../../ipc/commands";
import { ru } from "../../i18n/ru";
import { Controls, Cover, Equalizer, Progress } from "./parts";

/** Compact pill while music exists: cover on the left, equalizer on the right. */
export function MediaCompact({ media }: { media: MediaState }) {
  return (
    <div className="flex h-full items-center justify-between pr-4 pl-2">
      <Cover media={media} size={24} radius={6} />
      <Equalizer playing={media.isPlaying} className="text-[var(--island-fg)]" />
    </div>
  );
}

/** Peek on track change: cover, title, artist. */
export function MediaPeek({ media }: { media: MediaState }) {
  return (
    <div className="flex h-full items-center gap-3 pr-5 pl-3">
      <Cover media={media} size={48} radius={10} />
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm font-semibold">{media.title}</div>
        <div className="truncate text-xs text-[var(--island-muted)]">
          {media.artist || ru.media.unknownArtist}
        </div>
      </div>
      <Equalizer playing={media.isPlaying} />
    </div>
  );
}

/** Full-width player for the home tab. */
export function MediaPlayer({ media }: { media: MediaState }) {
  return (
    <section aria-label={ru.media.nowPlaying} className="flex w-full items-center gap-6">
      <Cover media={media} size={168} radius={18} />
      <div className="flex min-w-0 flex-1 flex-col gap-4">
        <div className="min-w-0">
          <div className="truncate text-xl font-semibold">{media.title}</div>
          <div className="truncate text-sm text-[var(--island-muted)]">
            {[media.artist || ru.media.unknownArtist, media.album].filter(Boolean).join(" — ")}
          </div>
        </div>
        <Progress media={media} />
        <Controls media={media} />
      </div>
    </section>
  );
}
