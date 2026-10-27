import type { Lang } from '#lib/i18n';
import type { LayoutLoad } from './$types';

export const load: LayoutLoad = ({ params }) => ({ lang: (params.lang ?? 'en') as Lang });
