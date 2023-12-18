const IMAGE_PROXY_URL = Bun.env.IMAGE_PROXY_URL;

if (!IMAGE_PROXY_URL) throw new Error("IMAGE_PROXY_URL is not set");

export const imageUrl = (url: string) => `${IMAGE_PROXY_URL}?url=${url}`;

export const description = (username: string) => `
This bot can help you find and share movies and tv shows\\.

It works automatically, no need to add it anywhere\\.

Simply open any of your chats, type \`@${username}\` and a movie or a tv show title after a space\\.
Then tap on one of the results\\. 

If you just type \`@${username}\` and a space, you'll see trending results\\.
    
If you already have some messages from this bot in your chat, you can just tap on bot's username and it will be pasted into the input field\\.
    
You can try it right here\\.`;
