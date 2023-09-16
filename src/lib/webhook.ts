import { webhookCallback } from 'grammy';
import { bot } from './bot';

export const handleUpdate = webhookCallback(bot, 'sveltekit');
