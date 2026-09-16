import type {
  GenSettings,
  Model,
  ModelCapabilities,
  PresetFamily,
  PresetVariant,
  Route,
} from "../types";

/**
 * What we know when we know nothing.
 *
 * Deliberately *empty* rather than plausible. The previous version assumed
 * durations [5, 8, 10] and resolutions ["720p", "1080p"] for every model, so
 * the chip row confidently offered 10s on a 5s-only model, the estimator
 * quoted 10s, the Generate button displayed that price, and the provider
 * rejected the job after the round trip. An empty axis renders no chip, which
 * is honest; a wrong chip costs the user money.
 */
export const UNKNOWN_CAPABILITIES: ModelCapabilities = {
  // Nothing is known, so nothing is ruled out — the same honesty as the empty
  // axes above. A slot wrongly greyed out is as bad as a chip wrongly offered.
  unsupportedRoles: [],
  supportsDuration: false,
  durations: [],
  supportsResolution: false,
  resolutions: [],
  supportsAspect: false,
  aspects: [],
  audio: false,
  constraints: [],
};

export function capabilitiesFor(model: Model | null): ModelCapabilities {
  return model?.capabilities ?? UNKNOWN_CAPABILITIES;
}

/**
 * Values a model will accept for an axis it supports but does not enumerate.
 *
 * Offered as a convenience ladder next to a free numeric input, not as the
 * truth about the model — which is why it is a separate function with a name
 * that says so, rather than a default folded into the capabilities.
 */
export function durationLadder(caps: ModelCapabilities): number[] {
  if (caps.durations.length > 0) return caps.durations;
  if (caps.durationRange) return ladderFromRange(caps.durationRange, caps.defaultDuration);
  if (!caps.supportsDuration) return [];
  const common = [3, 4, 5, 6, 8, 10, 12];
  const d = caps.defaultDuration;
  return d != null && !common.includes(d)
    ? [...common, d].sort((a, b) => a - b)
    : common;
}

/**
 * Whole-second chips across a free-form model's real range, thinned to at most
 * twelve so a 2–30s model does not print 29 chips, while always keeping both
 * endpoints and the default and never emitting a value outside the bound.
 *
 * A range is not an enum — the provider takes every integer between the bounds
 * — so this is a convenience ladder beside the numeric input, not the truth
 * about the model. It exists because the generic ladder stopped at 12 and hid
 * that MiniMax H3 Max renders up to 15 and Wan 3.0 Prime up to 30.
 */
export function ladderFromRange(
  range: [number, number],
  defaultDuration?: number | null,
): number[] {
  const min = Math.ceil(range[0]);
  const max = Math.floor(range[1]);
  if (max < min) return [];
  const all: number[] = [];
  for (let v = min; v <= max; v++) all.push(v);

  const MAX_CHIPS = 12;
  // A range that already fits needs no thinning; the default (when in range) is
  // already among the whole-second values.
  if (all.length <= MAX_CHIPS) return all;

  const d = defaultDuration;
  // The endpoints and the default (when in range) are mandatory — they consume
  // from the budget FIRST so adding the default can never push the ladder to a
  // thirteenth chip. Whatever budget is left is filled by an even stride.
  const kept = new Set<number>([min, max]);
  if (d != null && d >= min && d <= max) kept.add(d);
  const stride = (all.length - 1) / (MAX_CHIPS - 1);
  for (let i = 0; i < MAX_CHIPS && kept.size < MAX_CHIPS; i++) {
    kept.add(all[Math.round(i * stride)]);
  }
  return [...kept].sort((a, b) => a - b).slice(0, MAX_CHIPS);
}

/**
 * Route selection.
 *
 * A stored preference wins only if the model still exposes it — routes come
 * and go as providers delist models, and a stale preference must not pin the
 * UI to a route that can no longer run.
 */
export function selectRoute(
  model: Model | null,
  preferredRouteId?: string | null,
): Route | null {
  if (!model || model.routes.length === 0) return null;
  if (preferredRouteId) {
    const match = model.routes.find((r) => r.id === preferredRouteId);
    if (match) return match;
  }
  return model.routes[0];
}

export function routeLabel(route: Route | null | undefined): string {
  if (!route) return "No route";
  return `${route.provider} · ${route.slug}`;
}

/**
 * Preset variant selection.
 *
 * Exact model match first, then the family's own default (its first variant),
 * then null. The middle step matters: most families are authored against one
 * model and derived for the rest, and falling straight to null would hide
 * every preset the moment a user switched model.
 */
export function selectVariant(
  family: PresetFamily | null | undefined,
  modelId: string | null | undefined,
): PresetVariant | null {
  if (!family?.variants || family.variants.length === 0) return null;
  if (modelId) {
    const exact = family.variants.find((v) => v.modelId === modelId);
    if (exact) return exact;
  }
  return family.variants[0];
}

/** True when the family was authored for this model rather than derived. */
export function isNativeVariant(
  family: PresetFamily | null | undefined,
  modelId: string | null | undefined,
): boolean {
  if (!family?.variants || !modelId) return false;
  return family.variants.some((v) => v.modelId === modelId);
}

function nearest(value: number, options: number[]): number {
  return options.reduce((best, option) => {
    const d = Math.abs(option - value);
    const bestD = Math.abs(best - value);
    // Ties go to the smaller value: rounding a duration up silently costs the
    // user money, rounding it down does not.
    if (d < bestD || (d === bestD && option < best)) return option;
    return best;
  }, options[0]);
}

/**
 * Clamp settings to what the selected model can actually run, so switching
 * model never leaves an unsupported value staged for submit. Returns the same
 * object when nothing changed, which keeps this safe to call in a render path.
 */
export function resolveSettings(
  settings: GenSettings,
  caps: ModelCapabilities,
): GenSettings {
  // An enumerated axis clamps to its options. A free-form axis with a known
  // range clamps into that bound (so switching to H3 Max cannot leave 20s
  // staged); a free-form axis with no range is left alone, because clamping
  // against a list we invented is how a wrong value gets laundered into a
  // confident one.
  const duration =
    caps.durations.length > 0
      ? caps.durations.includes(settings.duration)
        ? settings.duration
        : nearest(settings.duration, caps.durations)
      : caps.durationRange
        ? Math.min(Math.max(settings.duration, caps.durationRange[0]), caps.durationRange[1])
        : settings.duration;

  // Prefer the model's own default over its first option — "the first one in
  // the list" is an arbitrary choice, and several models list their cheapest
  // or lowest-quality option first.
  const resolution = caps.resolutions.includes(settings.resolution)
    ? settings.resolution
    : (caps.defaultResolution ?? caps.resolutions[0] ?? settings.resolution);

  const aspect = caps.aspects.includes(settings.aspect)
    ? settings.aspect
    : (caps.defaultAspect ?? caps.aspects[0] ?? settings.aspect);

  const audio = caps.audio ? settings.audio : false;

  if (
    duration === settings.duration &&
    resolution === settings.resolution &&
    aspect === settings.aspect &&
    audio === settings.audio
  ) {
    return settings;
  }
  return { ...settings, duration, resolution, aspect, audio };
}
