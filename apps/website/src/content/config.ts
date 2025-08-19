import { defineCollection, z } from "astro:content";

const blog = defineCollection({
	type: "content",
	schema: z.object({
		title: z.string(),
		description: z.string().max(200).optional(),
		date: z.coerce.date(), // accepts string dates
		updated: z.coerce.date().optional(),
		draft: z.boolean().default(false),
		tags: z.array(z.string()).default([]),
		cover: z.string().optional(), // /public/... or remote URL
		canonical: z.string().url().optional(),
	}),
});

export const collections = { blog };
