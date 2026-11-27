// Ordenes de los desplegables "Sort". Los ids son los de escritorio (`store.sortSongs`,
// `sortLists`): se guardan en los ajustes.
import {
  ArrowDownAZ,
  ArrowDownZA,
  ArrowDownWideNarrow,
  ArrowDownNarrowWide,
  CalendarArrowDown,
  CalendarArrowUp,
  UserRound,
  ListOrdered,
  ListEnd,
  Hash,
  History,
  ClockArrowUp,
} from "@lucide/svelte";

export const SONG_SORTS = [
  { id: "date_desc", label: "Newest first", short: "Newest", icon: CalendarArrowDown },
  { id: "date_asc", label: "Oldest first", short: "Oldest", icon: CalendarArrowUp },
  { id: "title_asc", label: "Title (A–Z)", short: "A–Z", icon: ArrowDownAZ },
  { id: "title_desc", label: "Title (Z–A)", short: "Z–A", icon: ArrowDownZA },
  { id: "artist_asc", label: "Artist", short: "Artist", icon: UserRound },
  { id: "duration_desc", label: "Longest first", short: "Longest", icon: ArrowDownWideNarrow },
  { id: "duration_asc", label: "Shortest first", short: "Shortest", icon: ArrowDownNarrowWide },
];

/**
 * Las canciones de una lista: su orden, al reves (lo ultimo agregado arriba, salvo que se
 * haya reordenado en la PC) o los de siempre.
 */
export const LIST_SONG_SORTS = [
  { id: "custom", label: "List order", short: "List order", icon: ListOrdered },
  { id: "custom_rev", label: "List order, reversed", short: "Reversed", icon: ListEnd },
  ...SONG_SORTS,
];

export const LIST_SORTS = [
  { id: "name_asc", label: "Name (A–Z)", short: "A–Z", icon: ArrowDownAZ },
  { id: "name_desc", label: "Name (Z–A)", short: "Z–A", icon: ArrowDownZA },
  { id: "updated_desc", label: "Recently changed", short: "Recent", icon: History },
  { id: "updated_asc", label: "Least recently changed", short: "Oldest", icon: ClockArrowUp },
  { id: "count_desc", label: "Most songs", short: "Most songs", icon: Hash },
  { id: "count_asc", label: "Fewest songs", short: "Fewest", icon: Hash },
];
