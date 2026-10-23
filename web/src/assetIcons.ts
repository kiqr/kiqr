// Asset types name their icon in kebab-case (TYPE.yaml is data). A curated set keeps the bundle small;
// a type naming an icon outside it gets the generic file icon.
import {
  AudioLines, BookOpen, Camera, Clapperboard, File, FileText, Film, Gift, Image, Images, Laugh, LayoutTemplate, Mail, Megaphone, MessageCircle,
  MessageSquareText, Mic, Music, Newspaper, Palette, PenTool, Podcast, Presentation, Quote, Radio, Reply, Rss, Share2, Shapes, ShoppingBag, Smile,
  Sparkles, Star, Tag, Target, Trophy, Tv, Type, Video, type LucideIcon,
} from "lucide-react";

const ICONS: Record<string, LucideIcon> = {
  "audio-lines": AudioLines, "book-open": BookOpen, camera: Camera, clapperboard: Clapperboard, file: File, "file-text": FileText, film: Film, gift: Gift,
  image: Image, images: Images, laugh: Laugh, "layout-template": LayoutTemplate, mail: Mail, megaphone: Megaphone, "message-circle": MessageCircle,
  "message-square-text": MessageSquareText, mic: Mic, music: Music, newspaper: Newspaper, palette: Palette, "pen-tool": PenTool, podcast: Podcast,
  presentation: Presentation, quote: Quote, radio: Radio, reply: Reply, rss: Rss, "share-2": Share2, shapes: Shapes, "shopping-bag": ShoppingBag,
  smile: Smile, sparkles: Sparkles, star: Star, tag: Tag, target: Target, trophy: Trophy, tv: Tv, type: Type, video: Video,
};

export function assetIcon(name: string | undefined): LucideIcon {
  return (name && ICONS[name]) || File;
}
